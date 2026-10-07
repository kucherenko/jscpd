//! EDN report, `jscpd-report.edn`: every pair of structurally similar
//! functions `--similarity` found, most similar first, and the other clones
//! of the run.
//!
//! ```edn
//! {:candidates
//!  [{:score 0.890909090909
//!    :language "python"
//!    :left {:file "src/billing/invoice.py", :start-line 3, :end-line 13}
//!    :right {:file "src/billing/receipt.py", :start-line 3, :end-line 14}
//!    :left-nodes 158
//!    :right-nodes 166}]
//!  :clones
//!  [{:kind :exact
//!    :format "python"
//!    :left {:file "src/a.py", :start-line 10, :end-line 24}
//!    :right {:file "src/b.py", :start-line 4, :end-line 18}
//!    :tokens 96}]}
//! ```
//!
//! `:candidates` holds the pairs of `--similarity`, also those a token clone
//! already reports: `:score` is the Jaccard index of the two functions'
//! subtree fingerprints and `:left-nodes` and `:right-nodes` the size of
//! each normalized tree. `:clones` holds the clones of the other passes:
//! exact and renamed ones, gap-merged ones (`:kind :similar`, `:method
//! :gap`, with their `:score`) and semantic ones. Both are empty vectors
//! when a run finds nothing. A file in a code block of a Markdown file or a
//! component is the host file.

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, write_report_file};
use cpd_core::models::{CpdClone, Fragment, SimilarityMethod};
use std::path::Path;

pub struct EdnReporter {
    style: Style,
}

impl EdnReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            style: Style::new(opts.no_colors),
        }
    }
}

impl Reporter for EdnReporter {
    fn name(&self) -> &str {
        "edn"
    }

    fn report(
        &self,
        clones: &[CpdClone],
        ctx: &ReportContext,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let content = render(ctx.similar, clones);
        write_report_file(output_dir, "jscpd-report.edn", content, &self.style, "EDN")?;
        Ok(())
    }
}

/// The report for the pairs `similar` and the clones `clones`, of which the
/// `--similarity` pairs are left out: they are among `similar`.
pub fn render(similar: &[CpdClone], clones: &[CpdClone]) -> String {
    let mut candidates: Vec<&CpdClone> = similar.iter().filter(|c| c.structure.is_some()).collect();
    candidates.sort_by(|x, y| {
        let score = |c: &CpdClone| c.structure.as_ref().map_or(0.0, |s| s.score());
        let language = |c: &CpdClone| c.structure.as_ref().map(|s| s.language.clone());
        score(y)
            .total_cmp(&score(x))
            .then_with(|| language(x).cmp(&language(y)))
            .then_with(|| x.position_key().cmp(&y.position_key()))
    });
    let others: Vec<&CpdClone> = clones
        .iter()
        .filter(|c| c.similarity_method != Some(SimilarityMethod::Ast))
        .collect();
    let mut out = String::from("{:candidates [");
    for pair in &candidates {
        let Some(structure) = &pair.structure else {
            continue;
        };
        out.push('\n');
        out.push_str(&format!(
            " {{:score {}\n  :language {}\n  :left {}\n  :right {}\n  :left-nodes {}\n  :right-nodes {}}}",
            float(structure.score()),
            string(&structure.language),
            span(&pair.fragment_a),
            span(&pair.fragment_b),
            structure.nodes[0],
            structure.nodes[1],
        ));
    }
    out.push_str(if candidates.is_empty() { "]" } else { "\n]" });
    out.push_str("\n :clones [");
    for clone in &others {
        out.push_str(&format!(
            "\n {{:kind :{}\n  :format {}\n  :left {}\n  :right {}\n  :tokens {}",
            clone.kind.as_str(),
            string(&clone.format),
            span(&clone.fragment_a),
            span(&clone.fragment_b),
            clone.token_count,
        ));
        if let Some(method) = clone.similarity_method {
            out.push_str(&format!("\n  :method :{}", method.as_str()));
        }
        if let Some(score) = clone.similarity {
            out.push_str(&format!("\n  :score {}", float(f64::from(score))));
        }
        out.push('}');
    }
    out.push_str(if others.is_empty() { "]}\n" } else { "\n]}\n" });
    out
}

/// `{:file "…", :start-line 3, :end-line 13}`.
fn span(fragment: &Fragment) -> String {
    format!(
        "{{:file {}, :start-line {}, :end-line {}}}",
        string(host_file(&fragment.source_id)),
        fragment.start.line,
        fragment.end.line
    )
}

/// The file a fragment's source lies in: a code block (`<path>:<format>`)
/// belongs to its host file.
fn host_file(source_id: &str) -> &str {
    match source_id.rsplit_once(':') {
        Some((host, block))
            if host
                .rsplit(['/', '\\'])
                .next()
                .is_some_and(|name| name.contains('.'))
                && !block.is_empty()
                && block
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '#')) =>
        {
            host
        }
        _ => source_id,
    }
}

/// An EDN string.
fn string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A float with up to 12 decimals and at least one: `1.0`, `0.890909090909`.
fn float(value: f64) -> String {
    let text = format!("{value:.12}");
    let text = text.trim_end_matches('0');
    match text.strip_suffix('.') {
        Some(whole) => format!("{whole}.0"),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::models::{CloneKind, Location, StructuralMatch};

    fn fragment(file: &str, start: u32, end: u32) -> Fragment {
        Fragment::new(
            file,
            Location::new(start, 0, 0),
            Location::new(end, 0, 0),
            [0, 0],
        )
    }

    fn pair(a: &str, b: &str, shared: u32, total: u32) -> CpdClone {
        let mut clone = CpdClone::exact("python", fragment(a, 3, 13), fragment(b, 3, 14), 40);
        clone.kind = CloneKind::Similar;
        clone.similarity_method = Some(SimilarityMethod::Ast);
        clone.similarity = Some(shared as f32 / total as f32);
        clone.structure = Some(StructuralMatch {
            language: "python".to_string(),
            nodes: [158, 166],
            shared,
            total,
        });
        clone
    }

    #[test]
    fn an_empty_run_writes_empty_vectors() {
        assert_eq!(render(&[], &[]), "{:candidates []\n :clones []}\n");
    }

    #[test]
    fn candidates_come_most_similar_first_with_their_trees_and_clones_after() {
        let similar = [
            pair("src/a.py", "src/b.py", 49, 55),
            pair("docs/guide.md:python", "src/c.py", 10, 10),
        ];
        let exact = CpdClone::exact("go", fragment("x.go", 1, 9), fragment("y.go", 2, 10), 60);
        let report = render(&similar, &[exact, similar[0].clone()]);
        assert_eq!(
            report,
            "{:candidates [\n {:score 1.0\n  :language \"python\"\n  :left {:file \"docs/guide.md\", :start-line 3, :end-line 13}\n  :right {:file \"src/c.py\", :start-line 3, :end-line 14}\n  :left-nodes 158\n  :right-nodes 166}\n {:score 0.890909090909\n  :language \"python\"\n  :left {:file \"src/a.py\", :start-line 3, :end-line 13}\n  :right {:file \"src/b.py\", :start-line 3, :end-line 14}\n  :left-nodes 158\n  :right-nodes 166}\n]\n :clones [\n {:kind :exact\n  :format \"go\"\n  :left {:file \"x.go\", :start-line 1, :end-line 9}\n  :right {:file \"y.go\", :start-line 2, :end-line 10}\n  :tokens 60}\n]}\n"
        );
    }

    #[test]
    fn strings_are_escaped_and_block_suffixes_go() {
        assert_eq!(string("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(host_file("README.md:javascript"), "README.md");
        assert_eq!(host_file(r"C:\p\a.js"), r"C:\p\a.js");
        assert_eq!(host_file("src/a.py"), "src/a.py");
    }
}
