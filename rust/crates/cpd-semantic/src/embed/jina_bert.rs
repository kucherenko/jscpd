// jina_bert.rs — the JinaBERT v2 encoder behind jina-embeddings-v2-base-code,
// run on the CPU with candle.
//
// A BERT encoder with three changes: no position embeddings, ALiBi biases in
// the attention scores instead, which lets it read long inputs; the
// feed-forward is a gated linear unit (GEGLU); and in the `qk-post-norm`
// variant the code model uses, queries and keys pass a LayerNorm right after
// their projection. Each layer adds the attention output to its input twice —
// once inside the attention block, once around it — as the reference
// implementation does. The sentence embedding is the mean of the last hidden
// states over the real (unpadded) tokens.

use candle_core::{D, Device, Module, Result, Tensor};
use candle_nn::{Embedding, LayerNorm, Linear, VarBuilder};

/// What the model's `config.json` says about its shape.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub intermediate_size: usize,
    pub layer_norm_eps: f64,
    /// GELU (`geglu`) or ReLU (`reglu`) gate in the feed-forward.
    pub gelu_gate: bool,
    /// Whether queries and keys are layer-normed after projection.
    pub qk_norm: bool,
}

impl Config {
    /// Read a JinaBERT v2 `config.json`; anything else is refused.
    pub fn from_json(text: &str) -> std::result::Result<Self, String> {
        let v: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("config.json: {e}"))?;
        let int = |key: &str| {
            v.get(key)
                .and_then(serde_json::Value::as_u64)
                .map(|n| n as usize)
                .ok_or_else(|| format!("config.json: no {key}"))
        };
        let text_of = |key: &str| v.get(key).and_then(serde_json::Value::as_str).unwrap_or("");
        if text_of("position_embedding_type") != "alibi" {
            return Err("config.json: not a JinaBERT v2 model (no ALiBi)".to_string());
        }
        let gelu_gate = match text_of("feed_forward_type") {
            "geglu" => true,
            "reglu" => false,
            other => {
                return Err(format!(
                    "config.json: unsupported feed_forward_type '{other}'"
                ));
            }
        };
        Ok(Self {
            vocab_size: int("vocab_size")?,
            hidden_size: int("hidden_size")?,
            num_hidden_layers: int("num_hidden_layers")?,
            num_attention_heads: int("num_attention_heads")?,
            intermediate_size: int("intermediate_size")?,
            layer_norm_eps: v
                .get("layer_norm_eps")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(1e-12),
            gelu_gate,
            qk_norm: text_of("_name_or_path").contains("qk-post-norm"),
        })
    }
}

/// ALiBi slopes, one per head, as the reference computes them: a geometric
/// series for the largest power of two not above the head count, then every
/// other slope of the next power of two for the remaining heads.
pub fn alibi_slopes(heads: usize) -> Vec<f32> {
    fn power_of_two(n: usize) -> Vec<f64> {
        let start = 2f64.powf(-(2f64.powf(-((n as f64).log2() - 3.0))));
        (0..n).map(|i| start * start.powi(i as i32)).collect()
    }
    let slopes: Vec<f64> = if heads.is_power_of_two() {
        power_of_two(heads)
    } else {
        let closest = 1usize << (usize::BITS - 1 - heads.leading_zeros());
        let mut s = power_of_two(closest);
        s.extend(
            power_of_two(2 * closest)
                .into_iter()
                .step_by(2)
                .take(heads - closest),
        );
        s
    };
    slopes.into_iter().map(|s| s as f32).collect()
}

/// `[1, heads, len, len]` biases: `-slope * |i - j|`.
fn alibi_bias(slopes: &[f32], len: usize, device: &Device) -> Result<Tensor> {
    let mut data = Vec::with_capacity(slopes.len() * len * len);
    for &slope in slopes {
        for i in 0..len {
            for j in 0..len {
                data.push(-slope * i.abs_diff(j) as f32);
            }
        }
    }
    Tensor::from_vec(data, (1, slopes.len(), len, len), device)
}

struct SelfAttention {
    query: Linear,
    key: Linear,
    value: Linear,
    norm_q: Option<LayerNorm>,
    norm_k: Option<LayerNorm>,
    dense: Linear,
    norm_out: LayerNorm,
    heads: usize,
    head_dim: usize,
}

impl SelfAttention {
    fn load(vb: VarBuilder, cfg: &Config) -> Result<Self> {
        let h = cfg.hidden_size;
        let eps = cfg.layer_norm_eps;
        let own = vb.pp("self");
        let norm = |name: &str| {
            cfg.qk_norm
                .then(|| candle_nn::layer_norm(h, eps, own.pp(name)))
                .transpose()
        };
        Ok(Self {
            query: candle_nn::linear(h, h, own.pp("query"))?,
            key: candle_nn::linear(h, h, own.pp("key"))?,
            value: candle_nn::linear(h, h, own.pp("value"))?,
            norm_q: norm("layer_norm_q")?,
            norm_k: norm("layer_norm_k")?,
            dense: candle_nn::linear(h, h, vb.pp("output").pp("dense"))?,
            norm_out: candle_nn::layer_norm(h, eps, vb.pp("output").pp("LayerNorm"))?,
            heads: cfg.num_attention_heads,
            head_dim: h / cfg.num_attention_heads,
        })
    }

    fn forward(&self, x: &Tensor, alibi: &Tensor, mask: &Tensor) -> Result<Tensor> {
        let (b, len, hidden) = x.dims3()?;
        let project = |linear: &Linear, norm: &Option<LayerNorm>| -> Result<Tensor> {
            let t = linear.forward(x)?;
            let t = match norm {
                Some(norm) => norm.forward(&t)?,
                None => t,
            };
            t.reshape((b, len, self.heads, self.head_dim))?
                .transpose(1, 2)?
                .contiguous()
        };
        let q = project(&self.query, &self.norm_q)?;
        let k = project(&self.key, &self.norm_k)?;
        let v = project(&self.value, &None)?;
        let scores = (q.matmul(&k.t()?)? * (1.0 / (self.head_dim as f64).sqrt()))?;
        let scores = scores.broadcast_add(alibi)?.broadcast_add(mask)?;
        let probs = candle_nn::ops::softmax_last_dim(&scores)?;
        let context = probs
            .matmul(&v)?
            .transpose(1, 2)?
            .reshape((b, len, hidden))?;
        self.norm_out.forward(&(self.dense.forward(&context)? + x)?)
    }
}

struct Layer {
    attention: SelfAttention,
    norm_1: LayerNorm,
    up_gated: Linear,
    down: Linear,
    norm_2: LayerNorm,
    intermediate: usize,
    gelu_gate: bool,
}

impl Layer {
    fn load(vb: VarBuilder, cfg: &Config) -> Result<Self> {
        let (h, i, eps) = (cfg.hidden_size, cfg.intermediate_size, cfg.layer_norm_eps);
        Ok(Self {
            attention: SelfAttention::load(vb.pp("attention"), cfg)?,
            norm_1: candle_nn::layer_norm(h, eps, vb.pp("layer_norm_1"))?,
            up_gated: candle_nn::linear_no_bias(h, 2 * i, vb.pp("mlp").pp("up_gated_layer"))?,
            down: candle_nn::linear(i, h, vb.pp("mlp").pp("down_layer"))?,
            norm_2: candle_nn::layer_norm(h, eps, vb.pp("layer_norm_2"))?,
            intermediate: i,
            gelu_gate: cfg.gelu_gate,
        })
    }

    fn forward(&self, x: &Tensor, alibi: &Tensor, mask: &Tensor) -> Result<Tensor> {
        let attended = self.attention.forward(x, alibi, mask)?;
        let x = self.norm_1.forward(&(x + attended)?)?;
        let up_gated = self.up_gated.forward(&x)?;
        let up = up_gated.narrow(D::Minus1, 0, self.intermediate)?;
        let gate = up_gated.narrow(D::Minus1, self.intermediate, self.intermediate)?;
        let gate = match self.gelu_gate {
            true => gate.gelu_erf()?,
            false => gate.relu()?,
        };
        let mlp = self.down.forward(&(up * gate)?)?;
        self.norm_2.forward(&(x + mlp)?)
    }
}

pub struct JinaBert {
    words: Embedding,
    /// Row 0 of the token-type table: every token is of type 0.
    token_type: Tensor,
    norm: LayerNorm,
    layers: Vec<Layer>,
    slopes: Vec<f32>,
    device: Device,
}

/// Added to the scores of padding keys; `f32::MIN` as the reference does.
const MASKED: f64 = f32::MIN as f64;

impl JinaBert {
    pub fn load(vb: VarBuilder, cfg: &Config) -> Result<Self> {
        let emb = vb.pp("embeddings");
        let token_types = emb
            .pp("token_type_embeddings")
            .get((2, cfg.hidden_size), "weight")?;
        Ok(Self {
            words: candle_nn::embedding(
                cfg.vocab_size,
                cfg.hidden_size,
                emb.pp("word_embeddings"),
            )?,
            token_type: token_types.get(0)?,
            norm: candle_nn::layer_norm(cfg.hidden_size, cfg.layer_norm_eps, emb.pp("LayerNorm"))?,
            layers: (0..cfg.num_hidden_layers)
                .map(|n| Layer::load(vb.pp("encoder").pp("layer").pp(n), cfg))
                .collect::<Result<_>>()?,
            slopes: alibi_slopes(cfg.num_attention_heads),
            device: vb.device().clone(),
        })
    }

    /// Mean-pooled embeddings `[batch, hidden]` of `ids` (`[batch, len]`,
    /// padded) whose real tokens are 1 in `mask` (`[batch, len]`, f32).
    pub fn embed(&self, ids: &Tensor, mask: &Tensor) -> Result<Tensor> {
        let (b, len) = ids.dims2()?;
        let x = self.words.forward(ids)?.broadcast_add(&self.token_type)?;
        let mut x = self.norm.forward(&x)?;
        let alibi = alibi_bias(&self.slopes, len, &self.device)?;
        let padding = ((mask.affine(-1.0, 1.0)?) * MASKED)?.reshape((b, 1, 1, len))?;
        for layer in &self.layers {
            x = layer.forward(&x, &alibi, &padding)?;
        }
        let weights = mask.unsqueeze(2)?;
        let summed = x.broadcast_mul(&weights)?.sum(1)?;
        let counts = weights.sum(1)?.clamp(1e-9, f64::MAX)?;
        summed.broadcast_div(&counts)
    }
}

/// A two-layer model's config and fixed pseudo-random weights, for tests.
#[cfg(test)]
pub(crate) fn tiny_weights() -> (Config, std::collections::HashMap<String, Tensor>) {
    let cfg = Config {
        vocab_size: 11,
        hidden_size: 8,
        num_hidden_layers: 2,
        num_attention_heads: 2,
        intermediate_size: 6,
        layer_norm_eps: 1e-12,
        gelu_gate: true,
        qk_norm: true,
    };
    let mut seed = 7u32;
    let mut next = move || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        ((seed >> 16) % 1000) as f32 / 1000.0 - 0.5
    };
    let mut tensors: std::collections::HashMap<String, Tensor> = Default::default();
    let mut put = |name: String, shape: &[usize]| {
        let n: usize = shape.iter().product();
        let data: Vec<f32> = (0..n).map(|_| next()).collect();
        tensors.insert(name, Tensor::from_vec(data, shape, &Device::Cpu).unwrap());
    };
    let (h, i) = (cfg.hidden_size, cfg.intermediate_size);
    put(
        "embeddings.word_embeddings.weight".into(),
        &[cfg.vocab_size, h],
    );
    put("embeddings.token_type_embeddings.weight".into(), &[2, h]);
    for part in ["weight", "bias"] {
        put(format!("embeddings.LayerNorm.{part}"), &[h]);
    }
    for n in 0..cfg.num_hidden_layers {
        let p = format!("encoder.layer.{n}");
        for proj in ["query", "key", "value"] {
            put(format!("{p}.attention.self.{proj}.weight"), &[h, h]);
            put(format!("{p}.attention.self.{proj}.bias"), &[h]);
        }
        for norm in [
            "attention.self.layer_norm_q",
            "attention.self.layer_norm_k",
            "attention.output.LayerNorm",
            "layer_norm_1",
            "layer_norm_2",
        ] {
            put(format!("{p}.{norm}.weight"), &[h]);
            put(format!("{p}.{norm}.bias"), &[h]);
        }
        put(format!("{p}.attention.output.dense.weight"), &[h, h]);
        put(format!("{p}.attention.output.dense.bias"), &[h]);
        put(format!("{p}.mlp.up_gated_layer.weight"), &[2 * i, h]);
        put(format!("{p}.mlp.down_layer.weight"), &[h, i]);
        put(format!("{p}.mlp.down_layer.bias"), &[h]);
    }
    (cfg, tensors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::DType;

    #[test]
    fn slopes_match_the_reference_for_twelve_heads() {
        let s = alibi_slopes(12);
        let expected: Vec<f32> = (1..=8)
            .map(|k| 2f32.powi(-k))
            .chain([0.5f32, 1.5, 2.5, 3.5].map(|e| 2f32.powf(-e)))
            .collect();
        assert_eq!(s.len(), 12);
        for (got, want) in s.iter().zip(&expected) {
            assert!((got - want).abs() < 1e-7, "{s:?}");
        }
        assert_eq!(alibi_slopes(8), expected[..8].to_vec());
    }

    #[test]
    fn config_reads_the_code_model_and_refuses_others() {
        let text = r#"{"_name_or_path": "jinaai/jina-bert-v2-qk-post-norm", "position_embedding_type": "alibi",
            "feed_forward_type": "geglu", "vocab_size": 61056, "hidden_size": 768, "num_hidden_layers": 12,
            "num_attention_heads": 12, "intermediate_size": 3072, "layer_norm_eps": 1e-12}"#;
        let cfg = Config::from_json(text).unwrap();
        assert!(cfg.qk_norm && cfg.gelu_gate);
        assert_eq!((cfg.hidden_size, cfg.num_hidden_layers), (768, 12));
        let absolute = text.replace("alibi", "absolute");
        assert!(Config::from_json(&absolute).unwrap_err().contains("ALiBi"));
    }

    fn tiny() -> (JinaBert, Config) {
        let (cfg, tensors) = tiny_weights();
        let vb = VarBuilder::from_tensors(tensors, DType::F32, &Device::Cpu);
        (JinaBert::load(vb, &cfg).unwrap(), cfg)
    }

    #[test]
    fn padding_does_not_change_an_embedding() {
        let (model, cfg) = tiny();
        let alone = model
            .embed(
                &Tensor::new(&[[0u32, 5, 7, 2]], &Device::Cpu).unwrap(),
                &Tensor::new(&[[1f32, 1., 1., 1.]], &Device::Cpu).unwrap(),
            )
            .unwrap()
            .to_vec2::<f32>()
            .unwrap();
        let padded = model
            .embed(
                &Tensor::new(&[[0u32, 5, 7, 2, 1, 1], [0, 3, 2, 1, 1, 1]], &Device::Cpu).unwrap(),
                &Tensor::new(
                    &[[1f32, 1., 1., 1., 0., 0.], [1., 1., 1., 0., 0., 0.]],
                    &Device::Cpu,
                )
                .unwrap(),
            )
            .unwrap()
            .to_vec2::<f32>()
            .unwrap();
        assert_eq!(alone[0].len(), cfg.hidden_size);
        for (a, b) in alone[0].iter().zip(&padded[0]) {
            assert!((a - b).abs() < 1e-5, "{alone:?} vs {padded:?}");
        }
        assert_ne!(padded[0], padded[1]);
    }
}
