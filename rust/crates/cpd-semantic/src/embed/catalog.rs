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
//
// The near-best margin and the group floor of the rules were tuned with
// jina-embeddings-v2-base-code only. For the other models they are scaled
// from it by the model's gap between its two thresholds, which stands for
// how spread out its scores are: the margin is 0.05 per 0.15 of gap, and the
// floor sits as far above the same-language threshold (0.05 per 0.15).

use super::models::{self, LocalModel};
use crate::search::Thresholds;

/// An embedding model jscpd has calibrated.
#[derive(Debug)]
pub struct KnownModel {
    /// The name `--semantic-model` takes, and `--semantic-models` shows.
    pub name: &'static str,
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
        name: "CodeRankEmbed",
        id: models::CODERANKEMBED.id,
        ollama: &[],
        threshold: 0.4125,
        same_threshold: 0.6375,
        prefix: "",
        license: "MIT",
        local: Some(&models::CODERANKEMBED),
    },
    KnownModel {
        name: "jina-embeddings-v2-base-code",
        id: models::JINA_V2_BASE_CODE.id,
        ollama: &["unclemusclez/jina-embeddings-v2-base-code"],
        threshold: 0.6,
        same_threshold: 0.75,
        prefix: "",
        license: "Apache-2.0",
        local: Some(&models::JINA_V2_BASE_CODE),
    },
    KnownModel {
        name: "jina-code-embeddings-0.5b",
        id: "jinaai/jina-code-embeddings-0.5b",
        ollama: &[],
        threshold: 0.5625,
        same_threshold: 0.7125,
        prefix: "Candidate code snippet:\n",
        license: "CC-BY-NC-4.0",
        local: None,
    },
    KnownModel {
        name: "Qwen3-Embedding-0.6B",
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
        name: "SFR-Embedding-Code-400M_R",
        id: "Salesforce/SFR-Embedding-Code-400M_R",
        ollama: &[],
        threshold: 0.7375,
        same_threshold: 0.8375,
        prefix: "",
        license: "CC-BY-NC-4.0",
        local: None,
    },
    KnownModel {
        name: "gte-modernbert-base",
        id: "Alibaba-NLP/gte-modernbert-base",
        ollama: &[],
        threshold: 0.6875,
        same_threshold: 0.85,
        prefix: "",
        license: "Apache-2.0",
        local: None,
    },
    KnownModel {
        name: "codesage-small-v2",
        id: "codesage/codesage-small-v2",
        ollama: &[],
        threshold: 0.3125,
        same_threshold: 0.5625,
        prefix: "",
        license: "Apache-2.0",
        local: None,
    },
    KnownModel {
        name: "granite-embedding-english-r2",
        id: "ibm-granite/granite-embedding-english-r2",
        ollama: &[],
        threshold: 0.8625,
        same_threshold: 0.925,
        prefix: "",
        license: "Apache-2.0",
        local: None,
    },
    KnownModel {
        name: "bge-m3",
        id: "BAAI/bge-m3",
        ollama: &["bge-m3", "bge-m3:567m", "bge-m3:567m-fp16"],
        threshold: 0.7,
        same_threshold: 0.8375,
        prefix: "",
        license: "MIT",
        local: None,
    },
];

impl KnownModel {
    /// The thresholds of the rules on this model's scale: its calibrated
    /// two, and the near-best margin and group floor scaled from
    /// [`Thresholds::REFERENCE`].
    pub fn thresholds(&self) -> Thresholds {
        on_scale(self.threshold, self.same_threshold)
    }
}

/// The thresholds of a model whose calibrated thresholds are `across` and
/// `within`.
fn on_scale(across: f32, within: f32) -> Thresholds {
    let reference = Thresholds::REFERENCE;
    let spread = (within - across) / (reference.within - reference.across);
    Thresholds {
        across,
        within,
        near_best: reference.near_best * spread,
        group_floor: (within + (reference.group_floor - reference.within) * spread).min(1.0),
    }
}

/// The model `name` stands for: its name, its Hugging Face id or one of its
/// Ollama names, in any letter case, with or without Ollama's `:latest`
/// tag. A model of another owner under the same name is not this model.
pub fn find(name: &str) -> Option<&'static KnownModel> {
    let wanted = key(name);
    KNOWN_MODELS.iter().find(|m| {
        std::iter::once(m.name)
            .chain([m.id])
            .chain(m.ollama.iter().copied())
            .any(|known| key(known) == wanted)
    })
}

/// A name without `:latest`, in lower case.
fn key(name: &str) -> String {
    let name = name.trim();
    name.strip_suffix(":latest").unwrap_or(name).to_lowercase()
}

/// The thresholds of the rules for `model`: its calibrated ones, or
/// [`Thresholds::REFERENCE`] for a model jscpd does not know, with
/// `across` and `within` put in where they are set. `across` set alone
/// moves `within` along, keeping the model's gap between the two, at most
/// 1.
pub fn thresholds(model: &str, across: Option<f32>, within: Option<f32>) -> Thresholds {
    let own = find(model).map_or(Thresholds::REFERENCE, KnownModel::thresholds);
    let across_given = across.unwrap_or(own.across);
    Thresholds {
        across: across_given,
        within: within.unwrap_or(((across_given - own.across) + own.within).min(1.0)),
        ..own
    }
}

/// The names of the models jscpd runs itself, for messages.
pub fn local_names() -> String {
    KNOWN_MODELS
        .iter()
        .filter(|m| m.local.is_some())
        .map(|m| m.name)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_is_found_by_name_id_or_ollama_name() {
        for name in [
            "CodeRankEmbed",
            " coderankembed ",
            "nomic-ai/CodeRankEmbed",
            "NOMIC-AI/coderankembed",
        ] {
            assert_eq!(find(name).unwrap().id, "nomic-ai/CodeRankEmbed", "{name}");
        }
        let jina = find("unclemusclez/jina-embeddings-v2-base-code:latest").unwrap();
        assert_eq!(jina.name, "jina-embeddings-v2-base-code");
        assert_eq!(
            find("qwen3-embedding:0.6b").unwrap().name,
            "Qwen3-Embedding-0.6B"
        );
        assert_eq!(find("bge-m3:latest").unwrap().id, "BAAI/bge-m3");
        assert_eq!(
            find("jina-code-embeddings-0.5b").unwrap().id,
            "jinaai/jina-code-embeddings-0.5b"
        );
        // Another owner, size or quantization is another model.
        assert!(find("myorg/CodeRankEmbed").is_none());
        assert!(find("someone/bge-m3").is_none());
        assert!(find("qwen3-embedding:8b").is_none());
        assert!(find("nomic-embed-text").is_none());
        assert!(find("").is_none());
    }

    #[test]
    fn thresholds_follow_the_model_and_keep_its_gap() {
        let model = "CodeRankEmbed";
        let own = thresholds(model, None, None);
        assert_eq!((own.across, own.within), (0.4125, 0.6375));
        assert_eq!(thresholds(model, Some(0.4125), None), own);
        let moved = thresholds(model, Some(0.5), None);
        assert!((moved.within - 0.725).abs() < 1e-6, "{moved:?}");
        assert_eq!(
            thresholds(model, Some(0.9), None).within,
            1.0,
            "never above identical"
        );
        let both = thresholds(model, Some(0.5), Some(0.55));
        assert_eq!((both.across, both.within), (0.5, 0.55));
        assert_eq!(
            thresholds("some/unknown-model", None, None),
            Thresholds::REFERENCE
        );
    }

    #[test]
    fn the_group_rules_scale_with_the_model() {
        let close = |a: f32, b: f32| (a - b).abs() < 1e-6;
        let jina = find("jina-embeddings-v2-base-code").unwrap().thresholds();
        assert!(
            close(jina.near_best, 0.05) && close(jina.group_floor, 0.8),
            "{jina:?}"
        );
        // A gap of 0.225, one and a half times the reference's 0.15.
        let coderank = find("CodeRankEmbed").unwrap().thresholds();
        assert!(close(coderank.near_best, 0.075), "{coderank:?}");
        assert!(close(coderank.group_floor, 0.7125), "{coderank:?}");
        for m in KNOWN_MODELS {
            let t = m.thresholds();
            assert!(
                t.within < t.group_floor && t.group_floor <= 1.0,
                "{}: {t:?}",
                m.name
            );
            // At the model's own threshold, the other one is its own too.
            assert_eq!(
                thresholds(m.name, Some(m.threshold), None).within,
                m.same_threshold
            );
        }
    }

    #[test]
    fn every_model_is_sound() {
        let mut taken = std::collections::HashSet::new();
        for m in KNOWN_MODELS {
            assert!(0.0 < m.threshold && m.threshold < m.same_threshold && m.same_threshold <= 1.0);
            let names: std::collections::HashSet<String> = [m.name, m.id]
                .into_iter()
                .chain(m.ollama.iter().copied())
                .map(key)
                .collect();
            for name in names {
                assert!(taken.insert(name.clone()), "{name} names two models");
            }
            assert_eq!(
                m.id.rsplit('/').next(),
                Some(m.name),
                "the name is the repository's"
            );
            if let Some(local) = m.local {
                assert_eq!(local.id, m.id);
            }
        }
        assert_eq!(KNOWN_MODELS[0].name, super::super::DEFAULT_LOCAL_MODEL);
        assert_eq!(local_names(), "CodeRankEmbed, jina-embeddings-v2-base-code");
    }
}
