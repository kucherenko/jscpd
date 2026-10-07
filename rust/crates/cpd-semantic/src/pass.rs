//! `--semantic` as a clone pass of the finder, and the reader of functions
//! it shares with `--compare`.

use crate::search::{
    Embedder, SemanticParams, SemanticScope, SemanticUnit, Thresholds, UnitSource,
    find_semantic_clones,
};
use crate::units::{extract_units, supports_units};
use cpd_core::models::CpdClone;
use cpd_finder::pass::{ClonePass, PassContext, PassSource};
use cpd_similarity::test_files::is_test_path;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

/// Collects the functions of the files the finder shows it, one
/// [`UnitSource`] per detection source. [`SemanticPass`] reads files through
/// it; `--compare` uses it alone, with no clone detection.
#[derive(Default)]
pub struct UnitReader {
    sources: Mutex<Vec<UnitSource>>,
}

impl UnitReader {
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

impl ClonePass for UnitReader {
    /// Named after `--compare`, the one mode that runs it on its own;
    /// inside [`SemanticPass`] the pass's own name is used.
    fn name(&self) -> &'static str {
        "--compare"
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
                        let unit = SemanticUnit::build(
                            u.grammar,
                            u.name.clone(),
                            u.start.clone(),
                            u.end.clone(),
                            u.text.clone(),
                            source.spans,
                        )?;
                        Some(SemanticUnit {
                            test: u.test,
                            ..unit
                        })
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

    /// Finds nothing: the reader only collects.
    fn find(&self, _: &PassContext<'_>) -> Result<Vec<CpdClone>, String> {
        Ok(Vec::new())
    }
}

/// Finds semantic clones among the functions of the files the finder shows
/// it: reads each file's functions while the finder holds it, then embeds
/// and pairs them once the token passes are done.
pub struct SemanticPass {
    embedder: Arc<dyn Embedder>,
    thresholds: Thresholds,
    scope: SemanticScope,
    reader: UnitReader,
}

impl SemanticPass {
    /// `thresholds`: the similarities the rules need, on the scale of the
    /// embedder's model (see `SemanticOptions::thresholds`); `scope`: pairs
    /// within one language, across languages, or both.
    pub fn new(embedder: Arc<dyn Embedder>, thresholds: Thresholds, scope: SemanticScope) -> Self {
        Self {
            embedder,
            thresholds,
            scope,
            reader: UnitReader::default(),
        }
    }
}

impl ClonePass for SemanticPass {
    fn name(&self) -> &'static str {
        "--semantic"
    }

    fn reads(&self, format: &str) -> bool {
        self.reader.reads(format)
    }

    fn read(&self, format: &str, content: &str, sources: &[PassSource<'_>]) {
        self.reader.read(format, content, sources);
    }

    fn find(&self, context: &PassContext<'_>) -> Result<Vec<CpdClone>, String> {
        let mut sources = self.reader.take_sources();
        for source in &mut sources {
            source.path_label = (context.label)(&source.id);
        }
        if context.skip_tests {
            leave_out_tests(&mut sources);
        }
        let params = SemanticParams {
            thresholds: self.thresholds,
            min_tokens: context.min_tokens,
            min_lines: context.min_lines,
            scope: self.scope,
        };
        find_semantic_clones(&sources, self.embedder.as_ref(), &params, context.existing)
    }
}

/// `--similarity-skip-tests` for the semantic pairs: the functions of test
/// files and the tests among the code, as `--compare` tells them from code.
fn leave_out_tests(sources: &mut [UnitSource]) {
    for source in sources {
        let test_file = is_test_path(Path::new(&source.id));
        source.units.retain(|unit| !test_file && !unit.test);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::SemanticUnit;
    use cpd_core::detect::PathLabel;
    use cpd_core::models::Location;

    #[test]
    fn skipping_tests_leaves_out_test_files_and_tests_among_the_code() {
        let unit = |name: &str, test: bool| SemanticUnit {
            grammar: "oxc",
            name: name.to_string(),
            start: Location::new(1, 0, 0),
            end: Location::new(9, 0, 90),
            range: [0, 60],
            token_count: 60,
            text: name.to_string(),
            test,
        };
        let source = |id: &str, units: Vec<SemanticUnit>| UnitSource {
            id: id.to_string(),
            format: "typescript".to_string(),
            units,
            path_label: PathLabel::default(),
        };
        let mut sources = vec![
            source(
                "src/cart.ts",
                vec![unit("total", false), unit("adds", true)],
            ),
            source("src/cart.test.ts", vec![unit("helper", false)]),
        ];
        leave_out_tests(&mut sources);
        let left: Vec<&str> = sources
            .iter()
            .flat_map(|s| s.units.iter().map(|u| u.name.as_str()))
            .collect();
        assert_eq!(left, ["total"]);
    }
}
