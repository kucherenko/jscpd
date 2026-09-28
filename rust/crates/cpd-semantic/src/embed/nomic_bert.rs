// nomic_bert.rs — the NomicBERT encoder behind CodeRankEmbed, run on the CPU
// with candle.
//
// A BERT encoder with four changes: rotary position embeddings (RoPE) on the
// queries and keys instead of learned position embeddings; one fused
// projection for queries, keys and values; a gated feed-forward (SwiGLU:
// `fc2(fc11(x) * silu(fc12(x)))`); and no biases on any of those
// projections. LayerNorm follows each residual, as in BERT. The sentence
// embedding is the last hidden state of the first token ([CLS]), which is
// what the model's sentence-transformers pooling config selects.
//
// Only the variant CodeRankEmbed uses is read: SwiGLU, post-norm, rotary over
// the whole head, halves rotated (not interleaved), no rotary scaling.

use super::bert::{ConfigJson, Shape, TokenEmbeddings, padding_bias};
use candle_core::{Device, Module, Result, Tensor};
use candle_nn::{LayerNorm, Linear, VarBuilder};

/// What the model's `config.json` says.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub shape: Shape,
    pub rotary_base: f32,
}

impl Config {
    /// Read a NomicBERT `config.json` of the supported variant; anything
    /// else is refused with the setting that differs.
    pub fn from_json(text: &str) -> std::result::Result<Self, String> {
        let v = ConfigJson::parse(text)?;
        let flag = |key: &str| v.flag(key);
        if v.text("model_type") != "nomic_bert" {
            return Err("config.json: not a NomicBERT model".to_string());
        }
        let unsupported = [
            (
                v.text("activation_function") != "swiglu",
                "activation_function",
            ),
            (flag("prenorm") != Some(false), "prenorm"),
            (flag("causal") == Some(true), "causal"),
            (flag("use_rms_norm") == Some(true), "use_rms_norm"),
            (flag("parallel_block") == Some(true), "parallel_block"),
            (flag("qkv_proj_bias") != Some(false), "qkv_proj_bias"),
            (flag("mlp_fc1_bias") != Some(false), "mlp_fc1_bias"),
            (flag("mlp_fc2_bias") != Some(false), "mlp_fc2_bias"),
            (
                v.number("rotary_emb_fraction") != Some(1.0),
                "rotary_emb_fraction",
            ),
            (
                flag("rotary_emb_interleaved") == Some(true),
                "rotary_emb_interleaved",
            ),
            (v.is_set("rotary_scaling_factor"), "rotary_scaling_factor"),
        ];
        if let Some((_, key)) = unsupported.iter().find(|(differs, _)| *differs) {
            return Err(format!(
                "config.json: unsupported NomicBERT variant ({key})"
            ));
        }
        Ok(Self {
            shape: Shape {
                vocab_size: v.int("vocab_size")?,
                hidden_size: v.int("n_embd")?,
                num_hidden_layers: v.int("n_layer")?,
                num_attention_heads: v.int("n_head")?,
                intermediate_size: v.int("n_inner")?,
                layer_norm_eps: v.number("layer_norm_epsilon").unwrap_or(1e-12),
            },
            rotary_base: v.number("rotary_emb_base").unwrap_or(10_000.0) as f32,
        })
    }
}

/// `(cos, sin)` tables `[len, head_dim / 2]` for positions `0..len`,
/// computed in f32 as the reference does.
fn rotary_tables(
    base: f32,
    head_dim: usize,
    len: usize,
    device: &Device,
) -> Result<(Tensor, Tensor)> {
    let half = head_dim / 2;
    let inv_freq: Vec<f32> = (0..half)
        .map(|i| 1.0 / base.powf((2 * i) as f32 / head_dim as f32))
        .collect();
    let mut cos = Vec::with_capacity(len * half);
    let mut sin = Vec::with_capacity(len * half);
    for t in 0..len {
        for &f in &inv_freq {
            let angle = t as f32 * f;
            cos.push(angle.cos());
            sin.push(angle.sin());
        }
    }
    Ok((
        Tensor::from_vec(cos, (len, half), device)?,
        Tensor::from_vec(sin, (len, half), device)?,
    ))
}

struct Attention {
    qkv: Linear,
    out: Linear,
    heads: usize,
    head_dim: usize,
}

impl Attention {
    fn load(vb: VarBuilder, cfg: &Config) -> Result<Self> {
        let h = cfg.shape.hidden_size;
        Ok(Self {
            qkv: candle_nn::linear_no_bias(h, 3 * h, vb.pp("Wqkv"))?,
            out: candle_nn::linear_no_bias(h, h, vb.pp("out_proj"))?,
            heads: cfg.shape.num_attention_heads,
            head_dim: h / cfg.shape.num_attention_heads,
        })
    }

    fn forward(&self, x: &Tensor, cos: &Tensor, sin: &Tensor, mask: &Tensor) -> Result<Tensor> {
        let (b, len, hidden) = x.dims3()?;
        // The fused projection is laid out as [q | k | v], each head-major.
        let qkv = self
            .qkv
            .forward(x)?
            .reshape((b, len, 3, self.heads, self.head_dim))?;
        let part = |i: usize| -> Result<Tensor> {
            qkv.narrow(2, i, 1)?
                .squeeze(2)?
                .transpose(1, 2)?
                .contiguous()
        };
        let q = candle_nn::rotary_emb::rope(&part(0)?, cos, sin)?;
        let k = candle_nn::rotary_emb::rope(&part(1)?, cos, sin)?;
        let v = part(2)?;
        let scores = (q.matmul(&k.t()?)? * (1.0 / (self.head_dim as f64).sqrt()))?;
        let probs = candle_nn::ops::softmax_last_dim(&scores.broadcast_add(mask)?)?;
        let context = probs
            .matmul(&v)?
            .transpose(1, 2)?
            .reshape((b, len, hidden))?;
        self.out.forward(&context)
    }
}

struct Layer {
    attention: Attention,
    norm_1: LayerNorm,
    up: Linear,
    gate: Linear,
    down: Linear,
    norm_2: LayerNorm,
}

impl Layer {
    fn load(vb: VarBuilder, cfg: &Config) -> Result<Self> {
        let Shape {
            hidden_size: h,
            intermediate_size: i,
            layer_norm_eps: eps,
            ..
        } = cfg.shape;
        let mlp = vb.pp("mlp");
        Ok(Self {
            attention: Attention::load(vb.pp("attn"), cfg)?,
            norm_1: candle_nn::layer_norm(h, eps, vb.pp("norm1"))?,
            up: candle_nn::linear_no_bias(h, i, mlp.pp("fc11"))?,
            gate: candle_nn::linear_no_bias(h, i, mlp.pp("fc12"))?,
            down: candle_nn::linear_no_bias(i, h, mlp.pp("fc2"))?,
            norm_2: candle_nn::layer_norm(h, eps, vb.pp("norm2"))?,
        })
    }

    fn forward(&self, x: &Tensor, cos: &Tensor, sin: &Tensor, mask: &Tensor) -> Result<Tensor> {
        let attended = self.attention.forward(x, cos, sin, mask)?;
        let x = self.norm_1.forward(&(attended + x)?)?;
        let gated = (self.up.forward(&x)? * candle_nn::ops::silu(&self.gate.forward(&x)?)?)?;
        let mlp = self.down.forward(&gated)?;
        self.norm_2.forward(&(mlp + x)?)
    }
}

pub struct NomicBert {
    embeddings: TokenEmbeddings,
    layers: Vec<Layer>,
    rotary_base: f32,
    head_dim: usize,
    device: Device,
}

impl NomicBert {
    pub fn load(vb: VarBuilder, cfg: &Config) -> Result<Self> {
        let shape = &cfg.shape;
        Ok(Self {
            embeddings: TokenEmbeddings::load(&vb, vb.pp("emb_ln"), shape)?,
            layers: (0..shape.num_hidden_layers)
                .map(|n| Layer::load(vb.pp("encoder").pp("layers").pp(n), cfg))
                .collect::<Result<_>>()?,
            rotary_base: cfg.rotary_base,
            head_dim: shape.hidden_size / shape.num_attention_heads,
            device: vb.device().clone(),
        })
    }

    /// Embeddings `[batch, hidden]` of `ids` (`[batch, len]`, padded): the
    /// last hidden state of the first token. `mask` (`[batch, len]`, f32)
    /// is 1 for real tokens; padding keys get no attention.
    pub fn embed(&self, ids: &Tensor, mask: &Tensor) -> Result<Tensor> {
        let (_, len) = ids.dims2()?;
        let mut x = self.embeddings.forward(ids)?;
        let (cos, sin) = rotary_tables(self.rotary_base, self.head_dim, len, &self.device)?;
        let padding = padding_bias(mask)?;
        for layer in &self.layers {
            x = layer.forward(&x, &cos, &sin, &padding)?;
        }
        x.narrow(1, 0, 1)?.squeeze(1)
    }
}

/// A two-layer model's config and fixed pseudo-random weights, for tests.
#[cfg(test)]
pub(crate) fn tiny_weights() -> (Config, super::bert::Tensors) {
    let cfg = Config {
        shape: super::bert::TINY_SHAPE,
        rotary_base: 1000.0,
    };
    let (h, i) = (cfg.shape.hidden_size, cfg.shape.intermediate_size);
    let mut weights = super::bert::TestWeights::new();
    let mut put = |name: String, shape: &[usize]| weights.put(name, shape);
    put(
        "embeddings.word_embeddings.weight".into(),
        &[cfg.shape.vocab_size, h],
    );
    put("embeddings.token_type_embeddings.weight".into(), &[2, h]);
    for n in 0..cfg.shape.num_hidden_layers {
        let p = format!("encoder.layers.{n}");
        put(format!("{p}.attn.Wqkv.weight"), &[3 * h, h]);
        put(format!("{p}.attn.out_proj.weight"), &[h, h]);
        put(format!("{p}.mlp.fc11.weight"), &[i, h]);
        put(format!("{p}.mlp.fc12.weight"), &[i, h]);
        put(format!("{p}.mlp.fc2.weight"), &[h, i]);
    }
    for norm in (0..cfg.shape.num_hidden_layers)
        .flat_map(|n| ["norm1", "norm2"].map(|k| format!("encoder.layers.{n}.{k}")))
        .chain(["emb_ln".to_string()])
    {
        put(format!("{norm}.weight"), &[h]);
        put(format!("{norm}.bias"), &[h]);
    }
    (cfg, weights.tensors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> NomicBert {
        let (cfg, tensors) = tiny_weights();
        let vb = VarBuilder::from_tensors(tensors, candle_core::DType::F32, &Device::Cpu);
        NomicBert::load(vb, &cfg).unwrap()
    }

    #[test]
    fn config_reads_the_code_model_and_refuses_other_variants() {
        let code_model = include_str!("testdata/coderankembed-config.json");
        let cfg = Config::from_json(code_model).unwrap();
        assert_eq!(
            (
                cfg.shape.hidden_size,
                cfg.shape.num_hidden_layers,
                cfg.shape.num_attention_heads
            ),
            (768, 12, 12)
        );
        assert_eq!(
            (cfg.shape.intermediate_size, cfg.shape.vocab_size),
            (3072, 30528)
        );
        assert_eq!(cfg.rotary_base, 1000.0);
        let prenorm = code_model.replace("\"prenorm\": false", "\"prenorm\": true");
        assert!(Config::from_json(&prenorm).unwrap_err().contains("prenorm"));
        let jina = r#"{"model_type": "bert", "position_embedding_type": "alibi"}"#;
        assert!(
            Config::from_json(jina)
                .unwrap_err()
                .contains("not a NomicBERT")
        );
    }

    #[test]
    fn padding_does_not_change_an_embedding() {
        let bert = tiny();
        let one = |ids: &[u32], mask: &[f32]| {
            let n = ids.len();
            let ids = Tensor::from_vec(ids.to_vec(), (1, n), &Device::Cpu).unwrap();
            let mask = Tensor::from_vec(mask.to_vec(), (1, n), &Device::Cpu).unwrap();
            bert.embed(&ids, &mask)
                .unwrap()
                .to_vec2::<f32>()
                .unwrap()
                .remove(0)
        };
        let alone = one(&[4, 5, 6, 7], &[1.0; 4]);
        let padded = one(&[4, 5, 6, 7, 1, 1], &[1.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
        for (a, b) in alone.iter().zip(&padded) {
            assert!((a - b).abs() < 1e-5, "{alone:?} vs {padded:?}");
        }
        assert_ne!(alone, one(&[4, 5, 7, 6], &[1.0; 4]), "order matters: RoPE");
    }

    #[test]
    fn rotary_tables_start_at_the_identity() {
        let (cos, sin) = rotary_tables(1000.0, 4, 3, &Device::Cpu).unwrap();
        let (cos, sin) = (cos.to_vec2::<f32>().unwrap(), sin.to_vec2::<f32>().unwrap());
        assert_eq!(cos[0], vec![1.0, 1.0]);
        assert_eq!(sin[0], vec![0.0, 0.0]);
        // Position 1, frequency 1 / 1000^(2/4): angle 1/sqrt(1000).
        assert!((sin[1][1] - (1.0f32 / 1000f32.sqrt()).sin()).abs() < 1e-7);
    }
}
