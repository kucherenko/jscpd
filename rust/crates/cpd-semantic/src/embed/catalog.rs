// catalog.rs — the embedding models jscpd has calibrated, with the settings
// each one gets by default.
//
// Models score similarity on different scales: a pair of functions that one
// model scores 0.9 another scores 0.5, so thresholds tuned for one model do
// not carry over to another. Each model here got its thresholds on Rosetta
// Code, where the pairs to find are known: the similarities at which its
// precision equals that of jina-embeddings-v2-base-code at 0.6 across
// languages and 0.75 within one. The calibration put the prefix a model's
// card gives for code before every function, so jscpd does the same.

use super::models::{self, LocalModel};
use crate::search::SAME_LANGUAGE_MARGIN;

/// The threshold across languages for a model jscpd has not calibrated:
/// the median of the calibrated models' thresholds. The threshold within
/// one language is [`SAME_LANGUAGE_MARGIN`] above it.
pub const FALLBACK_THRESHOLD: f32 = 0.6;

/// An embedding model jscpd has calibrated.
#[derive(Debug)]
pub struct KnownModel {
    /// Hugging Face repository id.
    pub id: &'static str,
    /// Names Ollama serves the model under; the first is the one to pull.
    pub ollama: &'static [&'static str],
    /// Lowest cosine similarity of a pair across languages.
    pub threshold: f32,
    /// Lowest cosine similarity of a pair within one language.
    pub same_threshold: f32,
    /// Put before every function text.
    pub prefix: &'static str,
    pub license: &'static str,
    /// The files, when jscpd runs the model itself; the others need an
    /// embeddings API.
    pub local: Option<&'static LocalModel>,
}

/// The default model first, then the other one jscpd runs itself, then the
/// ones that need an embeddings API.
pub static KNOWN_MODELS: &[KnownModel] = &[
    KnownModel {
        id: models::CODERANKEMBED.id,
        ollama: &[],
        threshold: 0.4125,
        same_threshold: 0.6375,
        prefix: "",
        license: "MIT",
        local: Some(&models::CODERANKEMBED),
    },
    KnownModel {
        id: models::JINA_V2_BASE_CODE.id,
        ollama: &["unclemusclez/jina-embeddings-v2-base-code"],
        threshold: 0.6,
        same_threshold: 0.75,
        prefix: "",
        license: "Apache-2.0",
        local: Some(&models::JINA_V2_BASE_CODE),
    },
    KnownModel {
        id: "jinaai/jina-code-embeddings-0.5b",
        ollama: &[],
        threshold: 0.5625,
        same_threshold: 0.7125,
        prefix: "Candidate code snippet:\n",
        license: "CC-BY-NC-4.0",
        local: None,
    },
    KnownModel {
        id: "Qwen/Qwen3-Embedding-0.6B",
        ollama: &[
            "qwen3-embedding:0.6b",
            "qwen3-embedding:0.6b-fp16",
            "qwen3-embedding:0.6b-q8_0",
        ],
        threshold: 0.5875,
        same_threshold: 0.7625,
        prefix: "Instruct: Given a code snippet, retrieve code that implements the same functionality\nQuery:",
        license: "Apache-2.0",
        local: None,
    },
    KnownModel {
        id: "Salesforce/SFR-Embedding-Code-400M_R",
        ollama: &[],
        threshold: 0.7375,
        same_threshold: 0.8375,
        prefix: "",
        license: "CC-BY-NC-4.0",
        local: None,
    },
    KnownModel {
        id: "Alibaba-NLP/gte-modernbert-base",
        ollama: &[],
        threshold: 0.6875,
        same_threshold: 0.85,
        prefix: "",
        license: "Apache-2.0",
        local: None,
    },
    KnownModel {
        id: "codesage/codesage-small-v2",
        ollama: &[],
        threshold: 0.3125,
        same_threshold: 0.5625,
        prefix: "",
        license: "Apache-2.0",
        local: None,
    },
    KnownModel {
        id: "ibm-granite/granite-embedding-english-r2",
        ollama: &[],
        threshold: 0.8625,
        same_threshold: 0.925,
        prefix: "",
        license: "Apache-2.0",
        local: None,
    },
    KnownModel {
        id: "BAAI/bge-m3",
        ollama: &["bge-m3", "bge-m3:567m", "bge-m3:567m-fp16"],
        threshold: 0.7,
        same_threshold: 0.8375,
        prefix: "",
        license: "MIT",
        local: None,
    },
];

/// The model `name` stands for: its id, one of its Ollama names, or the
/// part after the last slash of either, in any letter case, with or
/// without Ollama's `:latest` tag. `CodeRankEmbed` is nomic-ai/CodeRankEmbed.
pub fn find(name: &str) -> Option<&'static KnownModel> {
    let wanted = key(name);
    KNOWN_MODELS
        .iter()
        .find(|m| key(m.id) == wanted || m.ollama.iter().any(|o| key(o) == wanted))
}

/// A name without its owner or namespace and without `:latest`, in lower
/// case.
fn key(name: &str) -> String {
    let name = name.trim();
    let name = name.strip_suffix(":latest").unwrap_or(name);
    name.rsplit('/').next().unwrap_or(name).to_lowercase()
}

/// The threshold across languages for `model`: its calibrated one, or
/// [`FALLBACK_THRESHOLD`].
pub fn default_threshold(model: &str) -> f32 {
    find(model).map_or(FALLBACK_THRESHOLD, |m| m.threshold)
}

/// The threshold within one language for `model` when only `threshold` is
/// set: `threshold` plus the gap between the model's two calibrated
/// thresholds ([`SAME_LANGUAGE_MARGIN`] for a model jscpd does not know),
/// at most 1. At the model's own threshold it is the model's own
/// same-language threshold.
pub fn default_same_threshold(model: &str, threshold: f32) -> f32 {
    match find(model) {
        Some(m) if threshold == m.threshold => m.same_threshold,
        Some(m) => (threshold + (m.same_threshold - m.threshold)).min(1.0),
        None => (threshold + SAME_LANGUAGE_MARGIN).min(1.0),
    }
}

/// The ids of the models jscpd runs itself, for messages.
pub fn local_names() -> String {
    KNOWN_MODELS
        .iter()
        .filter(|m| m.local.is_some())
        .map(|m| m.id)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_is_found_by_id_short_name_or_ollama_name() {
        let coderank = find("nomic-ai/CodeRankEmbed").unwrap();
        for name in [
            "coderankembed",
            " CodeRankEmbed ",
            "someone/CodeRankEmbed:latest",
        ] {
            assert_eq!(find(name).unwrap().id, coderank.id, "{name}");
        }
        let jina = find("unclemusclez/jina-embeddings-v2-base-code").unwrap();
        assert_eq!(jina.id, "jinaai/jina-embeddings-v2-base-code");
        assert_eq!(
            find("qwen3-embedding:0.6b").unwrap().id,
            "Qwen/Qwen3-Embedding-0.6B"
        );
        assert_eq!(find("bge-m3:latest").unwrap().id, "BAAI/bge-m3");
        // Another size or quantization is another model.
        assert!(find("qwen3-embedding:8b").is_none());
        assert!(find("nomic-embed-text").is_none());
        assert!(find("").is_none());
    }

    #[test]
    fn the_same_language_threshold_keeps_the_models_gap() {
        let model = "nomic-ai/CodeRankEmbed";
        assert_eq!(default_threshold(model), 0.4125);
        assert_eq!(default_same_threshold(model, 0.4125), 0.6375);
        assert!((default_same_threshold(model, 0.5) - 0.725).abs() < 1e-6);
        assert_eq!(
            default_same_threshold(model, 0.9),
            1.0,
            "never above identical"
        );
        assert_eq!(default_threshold("some/unknown-model"), FALLBACK_THRESHOLD);
        assert!((default_same_threshold("some/unknown-model", 0.6) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn every_model_is_sound() {
        let mut taken = std::collections::HashSet::new();
        for m in KNOWN_MODELS {
            assert!(0.0 < m.threshold && m.threshold < m.same_threshold && m.same_threshold <= 1.0);
            let names: std::collections::HashSet<String> = std::iter::once(m.id)
                .chain(m.ollama.iter().copied())
                .map(key)
                .collect();
            for name in names {
                assert!(taken.insert(name.clone()), "{name} names two models");
            }
            if let Some(local) = m.local {
                assert_eq!(local.id, m.id);
            }
        }
        assert_eq!(KNOWN_MODELS[0].id, super::super::DEFAULT_LOCAL_MODEL);
        assert_eq!(
            local_names(),
            "nomic-ai/CodeRankEmbed, jinaai/jina-embeddings-v2-base-code"
        );
    }
}
