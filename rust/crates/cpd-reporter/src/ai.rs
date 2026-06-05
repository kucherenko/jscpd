use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use cpd_core::models::CpdClone;
use std::path::Path;

pub struct AiReporter {
    no_colors: bool,
}

fn normalize_path(p: &str) -> String {
    p.replace('\\', "/")
}

fn format_range(start: u32, end: u32) -> String {
    format!("{}-{}", start, end)
}

fn compress_clone_line(path_a: &str, path_b: &str, range_a: &str, range_b: &str) -> String {
    let norm_a = normalize_path(path_a);
    let norm_b = normalize_path(path_b);

    if norm_a == norm_b {
        return format!("{} {} ~ {}", norm_a, range_a, range_b);
    }

    let parts_a: Vec<&str> = norm_a.split('/').collect();
    let parts_b: Vec<&str> = norm_b.split('/').collect();

    let min_len = parts_a.len().min(parts_b.len());
    let mut common_len = 0;
    while common_len < min_len.saturating_sub(1) && parts_a[common_len] == parts_b[common_len] {
        common_len += 1;
    }

    if common_len == 0 {
        return format!("{}:{} ~ {}:{}", norm_a, range_a, norm_b, range_b);
    }

    let prefix = parts_a[..common_len].join("/");
    let rem_a = parts_a[common_len..].join("/");
    let rem_b = parts_b[common_len..].join("/");
    format!("{}/ {}:{} ~ {}:{}", prefix, rem_a, range_a, rem_b, range_b)
}

impl AiReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            no_colors: opts.no_colors,
        }
    }

    fn bold(&self, text: &str) -> String {
        if self.no_colors {
            text.to_string()
        } else {
            format!("\x1b[1m{}\x1b[22m", text)
        }
    }
}

impl Reporter for AiReporter {
    fn name(&self) -> &str {
        "ai"
    }

    fn report(
        &self,
        clones: &[CpdClone],
        ctx: &ReportContext,
        _output_dir: &Path,
    ) -> Result<(), ReporterError> {
        println!("{}:", self.bold("Clones"));

        for clone in clones {
            let path_a = &clone.fragment_a.source_id;
            let path_b = &clone.fragment_b.source_id;
            let range_a = format_range(clone.fragment_a.start.line, clone.fragment_a.end.line);
            let range_b = format_range(clone.fragment_b.start.line, clone.fragment_b.end.line);
            println!(
                "{}",
                compress_clone_line(path_a, path_b, &range_a, &range_b)
            );
        }

        println!("---");
        println!(
            "{} clones · {} duplication",
            self.bold(&clones.len().to_string()),
            self.bold(&format!("{:.1}%", ctx.stats.total.percentage)),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compress_clone_line_same_path() {
        let result = compress_clone_line("src/a.js", "src/a.js", "10-20", "30-40");
        assert_eq!(result, "src/a.js 10-20 ~ 30-40");
    }

    #[test]
    fn compress_clone_line_different_roots() {
        let result = compress_clone_line("src/a.js", "lib/b.js", "10-20", "5-15");
        assert_eq!(result, "src/a.js:10-20 ~ lib/b.js:5-15");
    }

    #[test]
    fn compress_clone_line_common_prefix() {
        let result = compress_clone_line("src/foo/a.js", "src/bar/b.js", "10-20", "5-15");
        assert_eq!(result, "src/ foo/a.js:10-20 ~ bar/b.js:5-15");
    }

    #[test]
    fn compress_clone_line_deep_common_prefix() {
        let result =
            compress_clone_line("src/app/utils/a.ts", "src/app/utils/b.ts", "1-5", "10-15");
        assert_eq!(result, "src/app/utils/ a.ts:1-5 ~ b.ts:10-15");
    }

    #[test]
    fn normalize_path_backslashes() {
        assert_eq!(normalize_path("src\\foo\\a.js"), "src/foo/a.js");
    }

    #[test]
    fn normalize_path_forward_slashes() {
        assert_eq!(normalize_path("src/foo/a.js"), "src/foo/a.js");
    }

    #[test]
    fn format_range_works() {
        assert_eq!(format_range(10, 20), "10-20");
    }

    #[test]
    fn ai_reporter_name_is_ai() {
        let opts = ReporterOptions::new(std::path::PathBuf::from("/tmp"));
        assert_eq!(AiReporter::new(&opts).name(), "ai");
    }
}
