// local.rs — the `local` provider: a downloaded model run in-process on the
// CPU, so embedding makes no network call.

use super::Backend;
use super::jina_bert::{Config, JinaBert};
use super::models::LocalModel;
use candle_core::{DType, Device, Tensor};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Padding token of the model's tokenizer; masked out, so its value only
/// has to be a valid id.
const PAD: u32 = 1;
/// Attention scores of one batch, in floats: `batch * heads * len²` stays
/// under this, so a batch of long functions is small and one of short
/// functions is large.
const SCORE_BUDGET: usize = 64 * 1024 * 1024;
/// Tokens of one padded batch. Inputs are sorted by length, so a budget in
/// tokens keeps each batch's padding (computed, then thrown away) small.
const TOKEN_BUDGET: usize = 2048;
const MAX_BATCH: usize = 64;

pub struct LocalBackend {
    model: &'static LocalModel,
    dir: PathBuf,
    loaded: OnceLock<Result<Loaded, String>>,
}

struct Loaded {
    tokenizer: tokenizers::Tokenizer,
    bert: JinaBert,
    heads: usize,
}

impl LocalBackend {
    /// A backend for `model`, whose files are in `dir`. Nothing is read
    /// until the first function has to be embedded.
    pub fn new(model: &'static LocalModel, dir: PathBuf) -> Self {
        Self {
            model,
            dir,
            loaded: OnceLock::new(),
        }
    }

    fn loaded(&self) -> Result<&Loaded, String> {
        self.loaded
            .get_or_init(|| self.load())
            .as_ref()
            .map_err(Clone::clone)
    }

    fn load(&self) -> Result<Loaded, String> {
        let path = |name: &str| self.dir.join(name);
        let config_text = std::fs::read_to_string(path("config.json"))
            .map_err(|e| format!("{}: {e}", path("config.json").display()))?;
        let config = Config::from_json(&config_text)?;
        let mut tokenizer = tokenizers::Tokenizer::from_file(path("tokenizer.json"))
            .map_err(|e| format!("{}: {e}", path("tokenizer.json").display()))?;
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: self.model.max_tokens,
                ..Default::default()
            }))
            .map_err(|e| format!("tokenizer: {e}"))?;
        tokenizer.with_padding(None);
        // SAFETY: the weights file is only ever replaced by rename, never
        // written in place, so the mapping cannot change under us.
        let vb = unsafe {
            candle_nn::VarBuilder::from_mmaped_safetensors(
                &[path("model.safetensors")],
                DType::F32,
                &Device::Cpu,
            )
        }
        .map_err(|e| format!("{}: {e}", path("model.safetensors").display()))?;
        let bert = JinaBert::load(vb, &config).map_err(|e| format!("{}: {e}", self.model.id))?;
        Ok(Loaded {
            tokenizer,
            bert,
            heads: config.num_attention_heads,
        })
    }
}

impl Backend for LocalBackend {
    fn label(&self) -> String {
        format!("{} on this machine", self.model.id)
    }

    fn model_name(&self) -> &str {
        self.model.id
    }

    fn cache_identity(&self) -> serde_json::Value {
        serde_json::json!({
            "provider": "local",
            "model": self.model.id,
            "revision": self.model.revision,
            "max_tokens": self.model.max_tokens,
        })
    }

    fn embed(&self, texts: &[&str], progress: &dyn Fn(usize)) -> Result<Vec<Vec<f32>>, String> {
        let loaded = self.loaded()?;
        let encodings = loaded
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| format!("tokenizer: {e}"))?;
        let ids: Vec<&[u32]> = encodings.iter().map(|e| e.get_ids()).collect();
        // Shortest first, so each batch pads its inputs to about the same
        // length.
        let mut order: Vec<usize> = (0..ids.len()).collect();
        order.sort_by_key(|&i| (ids[i].len(), i));
        let mut out: Vec<Vec<f32>> = vec![Vec::new(); ids.len()];
        let mut start = 0;
        while start < order.len() {
            let mut end = start + 1;
            while end < order.len() && end - start < MAX_BATCH {
                let (rows, len) = (end - start + 1, ids[order[end]].len());
                if rows * len > TOKEN_BUDGET || rows * loaded.heads * len * len > SCORE_BUDGET {
                    break;
                }
                end += 1;
            }
            let batch = &order[start..end];
            let vectors = embed_batch(&loaded.bert, batch.iter().map(|&i| ids[i]))
                .map_err(|e| format!("{}: {e}", self.model.id))?;
            for (&i, v) in batch.iter().zip(vectors) {
                out[i] = v;
            }
            progress(end);
            start = end;
        }
        Ok(out)
    }
}

/// One padded batch through the model.
fn embed_batch<'a>(
    bert: &JinaBert,
    inputs: impl ExactSizeIterator<Item = &'a [u32]> + Clone,
) -> candle_core::Result<Vec<Vec<f32>>> {
    let rows = inputs.len();
    let len = inputs.clone().map(<[u32]>::len).max().unwrap_or(0);
    let mut ids = Vec::with_capacity(rows * len);
    let mut mask = Vec::with_capacity(rows * len);
    for input in inputs {
        ids.extend_from_slice(input);
        ids.extend(std::iter::repeat_n(PAD, len - input.len()));
        mask.extend(std::iter::repeat_n(1f32, input.len()));
        mask.extend(std::iter::repeat_n(0f32, len - input.len()));
    }
    let ids = Tensor::from_vec(ids, (rows, len), &Device::Cpu)?;
    let mask = Tensor::from_vec(mask, (rows, len), &Device::Cpu)?;
    bert.embed(&ids, &mask)?.to_vec2::<f32>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::models::LocalModel;

    static TINY: LocalModel = LocalModel {
        id: "test/tiny",
        revision: "0",
        files: &[],
        max_tokens: 5,
    };

    /// The files of a two-layer model with an eleven-word vocabulary.
    fn tiny_model_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jscpd-tiny-model-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (cfg, tensors) = super::super::jina_bert::tiny_weights();
        let config = serde_json::json!({
            "_name_or_path": "jinaai/jina-bert-v2-qk-post-norm",
            "position_embedding_type": "alibi",
            "feed_forward_type": "geglu",
            "vocab_size": cfg.vocab_size,
            "hidden_size": cfg.hidden_size,
            "num_hidden_layers": cfg.num_hidden_layers,
            "num_attention_heads": cfg.num_attention_heads,
            "intermediate_size": cfg.intermediate_size,
            "layer_norm_eps": cfg.layer_norm_eps,
        });
        std::fs::write(dir.join("config.json"), config.to_string()).unwrap();
        let words = [
            "<s>", "<pad>", "</s>", "[UNK]", "fn", "cart", "total", "price", "sum", "x", "y",
        ];
        let vocab: serde_json::Map<String, serde_json::Value> = words
            .iter()
            .enumerate()
            .map(|(i, w)| (w.to_string(), serde_json::json!(i)))
            .collect();
        let tokenizer = serde_json::json!({
            "version": "1.0",
            "truncation": null,
            "padding": null,
            "added_tokens": [],
            "normalizer": null,
            "pre_tokenizer": {"type": "Whitespace"},
            "post_processor": null,
            "decoder": null,
            "model": {"type": "WordLevel", "vocab": vocab, "unk_token": "[UNK]"}
        });
        std::fs::write(dir.join("tokenizer.json"), tokenizer.to_string()).unwrap();
        candle_core::safetensors::save(&tensors, dir.join("model.safetensors")).unwrap();
        dir
    }

    #[test]
    fn batches_padding_and_truncation_do_not_change_a_vector() {
        let backend = LocalBackend::new(&TINY, tiny_model_dir());
        let texts = [
            "cart total price",
            "sum x",
            "cart total price sum x y fn cart",
            "y",
        ];
        let calls = std::cell::Cell::new(0);
        let together = backend
            .embed(&texts, &|done| {
                calls.set(calls.get() + 1);
                assert!(done <= texts.len());
            })
            .unwrap();
        assert!(calls.get() >= 1);
        assert_eq!(together.len(), 4);
        assert!(together.iter().all(|v| v.len() == 8));
        for (text, batched) in texts.iter().zip(&together) {
            let alone = backend.embed(&[text], &|_| {}).unwrap().remove(0);
            for (a, b) in alone.iter().zip(batched) {
                assert!((a - b).abs() < 1e-5, "{text}: {alone:?} vs {batched:?}");
            }
        }
        // Five tokens at most: the long text embeds as its first five words.
        let head = backend.embed(&["cart total price sum x"], &|_| {}).unwrap();
        for (a, b) in head[0].iter().zip(&together[2]) {
            assert!((a - b).abs() < 1e-5, "{head:?} vs {:?}", together[2]);
        }
        assert_ne!(together[0], together[1]);
    }

    #[test]
    fn a_missing_file_is_named() {
        let backend = LocalBackend::new(&TINY, PathBuf::from("/nonexistent/jscpd-model"));
        let err = backend.embed(&["x"], &|_| {}).unwrap_err();
        assert!(err.contains("config.json"), "{err}");
    }
}
