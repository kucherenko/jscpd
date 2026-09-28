// bert.rs — what the BERT encoders jscpd runs have in common: their size
// in `config.json`, the token embeddings and the padding mask.

use candle_core::{Module, Result, Tensor};
use candle_nn::{Embedding, LayerNorm, VarBuilder};

/// The size of an encoder, as its `config.json` gives it.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub intermediate_size: usize,
    pub layer_norm_eps: f64,
    /// Rows of the token-type table; only type 0 is used.
    pub type_vocab_size: usize,
}

/// A parsed `config.json`, whose errors name the file.
pub struct ConfigJson(serde_json::Value);

impl ConfigJson {
    pub fn parse(text: &str) -> std::result::Result<Self, String> {
        serde_json::from_str(text)
            .map(Self)
            .map_err(|e| format!("config.json: {e}"))
    }

    pub fn int(&self, key: &str) -> std::result::Result<usize, String> {
        self.0
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as usize)
            .ok_or_else(|| format!("config.json: no {key}"))
    }

    /// The string at `key`, empty when there is none.
    pub fn text(&self, key: &str) -> &str {
        self.0
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
    }

    pub fn flag(&self, key: &str) -> Option<bool> {
        self.0.get(key).and_then(serde_json::Value::as_bool)
    }

    pub fn number(&self, key: &str) -> Option<f64> {
        self.0.get(key).and_then(serde_json::Value::as_f64)
    }

    /// The number at `key`, or `default` when there is none.
    pub fn int_or(&self, key: &str, default: usize) -> std::result::Result<usize, String> {
        match self.is_set(key) {
            true => self.int(key),
            false => Ok(default),
        }
    }

    /// Whether `key` holds anything but `null`.
    pub fn is_set(&self, key: &str) -> bool {
        self.0.get(key).is_some_and(|v| !v.is_null())
    }
}

/// Word embedding plus the embedding of token type 0, layer-normed.
pub struct TokenEmbeddings {
    words: Embedding,
    /// Row 0 of the token-type table: every token is of type 0.
    token_type: Tensor,
    norm: LayerNorm,
}

impl TokenEmbeddings {
    /// The tables under `embeddings` in `vb`, and the LayerNorm at `norm`.
    pub fn load(vb: &VarBuilder, norm: VarBuilder, shape: &Shape) -> Result<Self> {
        let emb = vb.pp("embeddings");
        let h = shape.hidden_size;
        let token_types = emb
            .pp("token_type_embeddings")
            .get((shape.type_vocab_size, h), "weight")?;
        Ok(Self {
            words: candle_nn::embedding(shape.vocab_size, h, emb.pp("word_embeddings"))?,
            token_type: token_types.get(0)?,
            norm: candle_nn::layer_norm(h, shape.layer_norm_eps, norm)?,
        })
    }

    /// `[batch, len, hidden]` for `ids` (`[batch, len]`).
    pub fn forward(&self, ids: &Tensor) -> Result<Tensor> {
        let x = self.words.forward(ids)?.broadcast_add(&self.token_type)?;
        self.norm.forward(&x)
    }
}

/// What the attention scores of padding keys get added; `f32::MIN`, as the
/// reference implementations do.
const MASKED: f64 = f32::MIN as f64;

/// The attention bias `[batch, 1, 1, len]` for `mask` (`[batch, len]`, f32,
/// 1 for real tokens): 0 for a real key, [`MASKED`] for padding.
pub fn padding_bias(mask: &Tensor) -> Result<Tensor> {
    let (b, len) = mask.dims2()?;
    ((mask.affine(-1.0, 1.0)?) * MASKED)?.reshape((b, 1, 1, len))
}

/// The size of the tiny models of the tests: two layers, an eleven-word
/// vocabulary.
#[cfg(test)]
pub(crate) const TINY_SHAPE: Shape = Shape {
    vocab_size: 11,
    hidden_size: 8,
    num_hidden_layers: 2,
    num_attention_heads: 2,
    intermediate_size: 6,
    layer_norm_eps: 1e-12,
    type_vocab_size: 2,
};

/// Named weights, as a safetensors file holds them.
#[cfg(test)]
pub(crate) type Tensors = std::collections::HashMap<String, Tensor>;

/// Fixed pseudo-random weights for the tiny models of the tests.
#[cfg(test)]
pub(crate) struct TestWeights {
    pub tensors: Tensors,
    seed: u32,
}

#[cfg(test)]
impl TestWeights {
    pub fn new() -> Self {
        Self {
            tensors: Default::default(),
            seed: 7,
        }
    }

    /// Add a tensor of `shape` filled with values in [-0.5, 0.5).
    pub fn put(&mut self, name: String, shape: &[usize]) {
        let n: usize = shape.iter().product();
        let data: Vec<f32> = (0..n)
            .map(|_| {
                self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                ((self.seed >> 16) % 1000) as f32 / 1000.0 - 0.5
            })
            .collect();
        let tensor = Tensor::from_vec(data, shape, &candle_core::Device::Cpu).unwrap();
        self.tensors.insert(name, tensor);
    }
}
