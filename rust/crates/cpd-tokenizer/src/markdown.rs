// markdown.rs
// Attribution: Markdown fenced code block extraction; inspired by jscpd-rs approach; rewritten independently.

use cpd_core::models::{Token, TokenKind};
use crate::tokenizer::Mode;

/// A parsed fenced code block.
#[derive(Debug, Clone)]
struct CodeFence {
    language: Option<String>,
    content: String,
    start_line: u32,
}

/// Extract all fenced code blocks from a Markdown document.
/// Supports both ``` and ~~~ fence styles.
fn extract_fences(source: &str) -> Vec<CodeFence> {
    let mut fences = Vec::new();
    let mut in_fence = false;
    let mut fence_char = '`';
    let mut fence_lang: Option<String> = None;
    let mut fence_content = String::new();
    let mut fence_start_line = 0u32;

    for (line_idx, line) in source.lines().enumerate() {
        let line_num = line_idx as u32 + 1;
        let trimmed = line.trim_start();

        if !in_fence {
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                let fc = trimmed.chars().next().unwrap_or('`');
                let rest = trimmed.trim_start_matches(fc).trim();
                fence_lang = if rest.is_empty() { None } else { Some(rest.to_string()) };
                fence_char = fc;
                in_fence = true;
                fence_content.clear();
                fence_start_line = line_num + 1;
            }
            // else: prose — skip
        } else {
            // Check for closing fence
            if trimmed.starts_with(&fence_char.to_string().repeat(3)) {
                fences.push(CodeFence {
                    language: fence_lang.take(),
                    content: fence_content.clone(),
                    start_line: fence_start_line,
                });
                fence_content.clear();
                in_fence = false;
            } else {
                fence_content.push_str(line);
                fence_content.push('\n');
            }
        }
    }

    fences
}

/// Tokenize a Markdown file: extract fenced code blocks and tokenize each.
/// Non-code prose is not tokenized.
pub fn tokenize_markdown(source: &str, mode: Mode) -> Vec<Token> {
    if source.is_empty() {
        return Vec::new();
    }

    // Collect jscpd:ignore-start/end ranges from outer Markdown prose
    let mut in_ignore = false;
    let mut ignore_ranges: Vec<(u32, u32)> = Vec::new();
    let mut ignore_start = 0u32;

    for (line_idx, line) in source.lines().enumerate() {
        let line_num = line_idx as u32 + 1;
        if line.contains("jscpd:ignore-start") {
            in_ignore = true;
            ignore_start = line_num;
        } else if line.contains("jscpd:ignore-end") && in_ignore {
            ignore_ranges.push((ignore_start, line_num));
            in_ignore = false;
        }
    }

    let fences = extract_fences(source);
    let mut all_tokens = Vec::new();

    for fence in &fences {
        let in_outer_ignore = ignore_ranges.iter().any(|(start, end)| {
            fence.start_line >= *start && fence.start_line <= *end
        });

        let format = fence.language.as_deref().unwrap_or("text");
        let mut fence_tokens = crate::tokenizer::tokenize(format, &fence.content, mode);

        let line_offset = fence.start_line.saturating_sub(1);
        for token in &mut fence_tokens {
            token.start.line += line_offset;
            token.end.line += line_offset;
            if in_outer_ignore {
                token.kind = TokenKind::Ignore;
            }
        }

        all_tokens.extend(fence_tokens);
    }

    all_tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::Mode;
    use cpd_core::models::TokenKind;

    const MD_WITH_JS: &str = "# Header\n\nSome prose.\n\n```javascript\nfunction hello() { return 42; }\n```\n\nMore prose.\n";
    const MD_NO_FENCES: &str = "# Just a Header\n\nSome plain text with no code.\n";
    const MD_UNKNOWN_LANG: &str = "```unknownlang999\nhello world\n```\n";
    const MD_WITH_IGNORE: &str = "<!-- jscpd:ignore-start -->\n```javascript\nconst x = 1;\n```\n<!-- jscpd:ignore-end -->\n```javascript\nconst y = 2;\n```\n";

    #[test]
    fn js_fence_produces_tokens() {
        let tokens = tokenize_markdown(MD_WITH_JS, Mode::Mild);
        assert!(!tokens.is_empty(), "JS code fence must produce tokens");
    }

    #[test]
    fn no_fences_produces_empty() {
        let tokens = tokenize_markdown(MD_NO_FENCES, Mode::Mild);
        assert!(tokens.is_empty(), "Markdown with no fences must produce no tokens");
    }

    #[test]
    fn unknown_lang_fence_does_not_panic() {
        let result = std::panic::catch_unwind(|| tokenize_markdown(MD_UNKNOWN_LANG, Mode::Mild));
        assert!(result.is_ok(), "unknown language fence must not panic");
    }

    #[test]
    fn empty_markdown_returns_empty() {
        let tokens = tokenize_markdown("", Mode::Mild);
        assert!(tokens.is_empty());
    }

    #[test]
    fn ignore_region_suppresses_fence_tokens() {
        let tokens = tokenize_markdown(MD_WITH_IGNORE, Mode::Mild);
        let non_ignore = tokens.iter().filter(|t| t.kind != TokenKind::Ignore).count();
        let ignore_count = tokens.iter().filter(|t| t.kind == TokenKind::Ignore).count();
        assert!(ignore_count > 0, "tokens in ignore region must be Ignore kind");
        assert!(non_ignore > 0, "tokens outside ignore region must NOT be Ignore kind");
    }
}
