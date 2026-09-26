//! `--semantic` as a clone pass of the finder.

use crate::search::{
    Embedder, SemanticParams, SemanticScope, SemanticUnit, UnitSource, find_semantic_clones,
};
use crate::units::{extract_units, supports_units};
use cpd_core::models::CpdClone;
use cpd_finder::pass::{ClonePass, PassContext, PassSource};
use std::sync::{Arc, Mutex, MutexGuard};

/// Finds semantic clones among the functions of the files the finder shows
/// it: reads each file's functions while the finder holds it, then embeds
/// and pairs them once the token passes are done.
pub struct SemanticPass {
    embedder: Arc<dyn Embedder>,
    threshold: f32,
    scope: SemanticScope,
    sources: Mutex<Vec<UnitSource>>,
}

impl SemanticPass {
    /// `threshold`: lowest cosine similarity reported; `scope`: pairs
    /// within one language, across languages, or both.
    pub fn new(embedder: Arc<dyn Embedder>, threshold: f32, scope: SemanticScope) -> Self {
        Self {
            embedder,
            threshold,
            scope,
            sources: Mutex::new(Vec::new()),
        }
    }

    /// The functions read since the last call, in source-id order, so that
    /// the result does not depend on which worker read which file.
    pub fn take_sources(&self) -> Vec<UnitSource> {
        let mut sources = std::mem::take(&mut *self.lock());
        sources.sort_by(|a, b| a.id.cmp(&b.id));
        sources
    }

    fn lock(&self) -> MutexGuard<'_, Vec<UnitSource>> {
        // A worker that panicked while pushing leaves whole entries only.
        self.sources
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl ClonePass for SemanticPass {
    fn name(&self) -> &'static str {
        "--semantic"
    }

    fn reads(&self, format: &str) -> bool {
        supports_units(format)
    }

    fn read(&self, format: &str, content: &str, sources: &[PassSource<'_>]) {
        let maps = extract_units(content, format);
        let found: Vec<UnitSource> = sources
            .iter()
            .filter_map(|source| {
                // A component's script blocks of one format form one map;
                // each detection source keeps the functions inside it.
                let map = maps.iter().find(|m| m.format == source.format)?;
                let units: Vec<SemanticUnit> = map
                    .units
                    .iter()
                    .filter_map(|u| {
                        SemanticUnit::build(
                            u.grammar,
                            u.name.clone(),
                            u.start.clone(),
                            u.end.clone(),
                            u.text.clone(),
                            source.spans,
                        )
                    })
                    .collect();
                (!units.is_empty()).then(|| UnitSource {
                    id: source.id.to_string(),
                    format: source.format.to_string(),
                    units,
                    path_label: Default::default(),
                })
            })
            .collect();
        if !found.is_empty() {
            self.lock().extend(found);
        }
    }

    fn find(&self, context: &PassContext<'_>) -> Result<Vec<CpdClone>, String> {
        let mut sources = self.take_sources();
        for source in &mut sources {
            source.path_label = (context.label)(&source.id);
        }
        let params = SemanticParams {
            threshold: self.threshold,
            min_tokens: context.min_tokens,
            min_lines: context.min_lines,
            scope: self.scope,
        };
        find_semantic_clones(&sources, self.embedder.as_ref(), &params, context.existing)
    }
}
