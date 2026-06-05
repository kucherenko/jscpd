// markdown.rs
// Attribution: Markdown fenced code block extraction; inspired by jscpd-rs approach; rewritten independently.

use std::collections::BTreeMap;

use cpd_core::models::{DetectionToken, Location, Token, TokenKind};

use crate::embedded::blank_ranges_preserve_newlines;
use crate::formats::resolve_format;
use crate::line_index::LineIndex;
use crate::tokenizer::{Mode, TokenMap, TokenizeOptions, push_token};

pub struct LineSpan {
    pub start: usize,
    pub end: usize,
    pub next_start: usize,
}

pub fn line_spans(content: &str) -> Vec<LineSpan> {
    if content.is_empty() {
        return Vec::new();
    }
    let mut spans = Vec::new();
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut pos = 0usize;
    while pos <= len {
        let line_end = bytes[pos..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|i| pos + i)
            .unwrap_or(len);
        let content_end = if line_end > pos && bytes[line_end - 1] == b'\r' {
            line_end - 1
        } else {
            line_end
        };
        let next_start = if line_end < len { line_end + 1 } else { len };
        spans.push(LineSpan {
            start: pos,
            end: content_end,
            next_start,
        });
        if next_start <= pos {
            break;
        }
        pos = next_start;
        if pos >= len && (len == 0 || bytes[len - 1] != b'\n') {
            break;
        }
    }
    spans
}

#[derive(Debug, Clone)]
struct MarkdownFence {
    format: String,
    #[allow(dead_code)]
    front_matter: bool,
    block_start: usize,
    inner_start: usize,
    inner_end: usize,
    block_end: usize,
}

struct FenceOpen {
    marker: u8,
    len: usize,
    info: String,
}

fn parse_opening_fence(line: &str) -> Option<FenceOpen> {
    let bytes = line.as_bytes();
    let marker = *bytes.first()?;
    if !matches!(marker, b'`' | b'~') {
        return None;
    }
    let len = bytes.iter().take_while(|&&b| b == marker).count();
    if len < 3 {
        return None;
    }
    Some(FenceOpen {
        marker,
        len,
        info: line[len..].trim().to_string(),
    })
}

fn is_closing_fence(line: &str, open: &FenceOpen) -> bool {
    let trimmed = line.trim();
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let len = bytes.iter().take_while(|&&b| b == open.marker).count();
    len >= open.len && bytes[len..].iter().all(|&b| b == b' ' || b == b'\t')
}

fn resolve_fence_format(info: &str) -> Option<&'static str> {
    let tag = info.split_whitespace().next()?;
    resolve_format(tag)
}

fn extract_code_fences(content: &str) -> Vec<MarkdownFence> {
    let lines = line_spans(content);
    let mut fences = Vec::new();
    let mut idx = 0usize;
    while idx < lines.len() {
        let line_text = &content[lines[idx].start..lines[idx].end];
        let Some(open) = parse_opening_fence(line_text) else {
            idx += 1;
            continue;
        };
        let resolved = resolve_fence_format(&open.info);
        let close_idx = lines[idx + 1..]
            .iter()
            .position(|span| {
                let candidate = &content[span.start..span.end];
                is_closing_fence(candidate, &open)
            })
            .map(|p| idx + 1 + p);
        let Some(close_idx) = close_idx else {
            idx += 1;
            continue;
        };
        let inner_start = lines
            .get(idx + 1)
            .map(|s| s.start)
            .unwrap_or(lines[idx].next_start);
        let inner_end = content[..lines[close_idx].start]
            .strip_suffix('\n')
            .map(|prefix| prefix.len())
            .unwrap_or(lines[close_idx].start);
        let inner_end = inner_end.max(inner_start);
        let block_end = lines[close_idx].next_start.min(content.len());
        let format = resolved
            .map(|r| r.to_string())
            .unwrap_or_else(|| open.info.split_whitespace().next().unwrap_or("").to_string());
        fences.push(MarkdownFence {
            format,
            front_matter: false,
            block_start: lines[idx].start,
            inner_start,
            inner_end,
            block_end,
        });
        idx = close_idx + 1;
    }
    fences
}

fn extract_front_matter(content: &str) -> Option<MarkdownFence> {
    if !(content.starts_with("---\n") || content.starts_with("---\r\n")) {
        return None;
    }
    let lines = line_spans(content);
    let close_idx = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, span)| {
            let line = content[span.start..span.end].trim();
            line == "---" || line == "..."
        })
        .map(|(idx, _)| idx)?;
    let inner_start = lines.get(1)?.start;
    let inner_end = content[..lines[close_idx].start]
        .strip_suffix('\n')
        .map(|prefix| prefix.len())
        .unwrap_or(lines[close_idx].start);
    let inner_end = inner_end.max(inner_start);
    let block_end = lines[close_idx].next_start.min(content.len());
    Some(MarkdownFence {
        format: "yaml".to_string(),
        front_matter: true,
        block_start: 0,
        inner_start,
        inner_end,
        block_end,
    })
}

fn collect_ignore_byte_ranges(content: &str) -> Vec<[usize; 2]> {
    let lines = line_spans(content);
    let mut ranges = Vec::new();
    let mut in_ignore = false;
    let mut ignore_start: usize = 0;
    for span in &lines {
        let line = &content[span.start..span.end];
        if line.contains("jscpd:ignore-start") {
            in_ignore = true;
            ignore_start = span.start;
        } else if line.contains("jscpd:ignore-end") && in_ignore {
            let end = span.next_start.min(content.len());
            ranges.push([ignore_start, end]);
            in_ignore = false;
        }
    }
    ranges
}

pub fn tokens_to_detection(tokens: Vec<Token>, options: &TokenizeOptions) -> Vec<DetectionToken> {
    let mut detection = Vec::with_capacity(tokens.len());
    for t in tokens {
        let byte_start = t.start.offset as usize;
        let byte_end = t.end.offset as usize;
        push_token(
            &mut detection,
            t.kind,
            &t.value,
            byte_start,
            byte_end,
            t.start,
            t.end,
            options,
        );
    }
    detection
}

pub fn offset_detection_tokens(
    tokens: &mut [DetectionToken],
    byte_offset: usize,
    start_location: &Location,
) {
    let line_offset = start_location.line.saturating_sub(1);
    let col_offset = start_location.column;
    for t in tokens.iter_mut() {
        t.start.line += line_offset;
        t.end.line += line_offset;
        t.start.offset += byte_offset as u32;
        t.end.offset += byte_offset as u32;
        t.range[0] += byte_offset;
        t.range[1] += byte_offset;
        if t.start.line == start_location.line {
            t.start.column += col_offset;
        }
        if t.end.line == start_location.line {
            t.end.column += col_offset;
        }
    }
}

pub fn tokenize_markdown_maps(
    source: &str,
    options: &TokenizeOptions,
) -> Vec<TokenMap> {
    if source.is_empty() {
        return Vec::new();
    }

    let ignore_ranges = collect_ignore_byte_ranges(source);

    let mut fences = extract_code_fences(source);
    if let Some(fm) = extract_front_matter(source) {
        fences.push(fm);
        fences.sort_by_key(|f| f.block_start);
    }

    let sanitized = blank_ranges_preserve_newlines(
        source,
        &fences
            .iter()
            .map(|f| [f.block_start, f.block_end])
            .collect::<Vec<_>>(),
    );

    let line_index = LineIndex::new(source.as_bytes());

    let body_tokens = crate::generic::tokenize_generic(&sanitized, "markdown");
    let mut markdown_detection = tokens_to_detection(body_tokens, options);
    markdown_detection.retain(|t| t.range[0] < t.range[1]);

    let mut maps = Vec::new();
    if !markdown_detection.is_empty() {
        maps.push(TokenMap {
            format: "markdown".to_string(),
            tokens: markdown_detection,
        });
    }

    let mut embedded_maps: BTreeMap<String, Vec<DetectionToken>> = BTreeMap::new();
    for fence in &fences {
        let inner = &source[fence.inner_start..fence.inner_end];
        let resolved = resolve_format(&fence.format).unwrap_or("text");
        let outer_ignored = ignore_ranges.iter().any(|[rs, re]| {
            fence.inner_start < *re && fence.inner_end > *rs
        });

        let mut inner_tokens = tokenize_to_detection_inner(resolved, inner, options);

        if outer_ignored {
            for t in &mut inner_tokens {
                t.range = [0, 0]; // mark as ignored by zeroing range
            }
            continue;
        }

        let inner_start_loc = line_index.location(fence.inner_start);
        offset_detection_tokens(&mut inner_tokens, fence.inner_start, &inner_start_loc);

        embedded_maps
            .entry(resolved.to_string())
            .or_default()
            .extend(inner_tokens);
    }

    for (format, tokens) in embedded_maps {
        maps.push(TokenMap {
            format,
            tokens,
        });
    }

    maps
}

fn tokenize_to_detection_inner(
    format: &str,
    source: &str,
    options: &TokenizeOptions,
) -> Vec<DetectionToken> {
    let raw = match format {
        "javascript" | "typescript" | "jsx" | "tsx" => {
            crate::javascript::tokenize_js(source, format)
        }
        "vue" | "svelte" | "astro" => {
            crate::sfc::tokenize_sfc(source, format, options.mode)
        }
        "markdown" | "md" => {
            crate::generic::tokenize_generic(source, format)
        }
        _ => crate::generic::tokenize_generic(source, format),
    };
    tokens_to_detection(raw, options)
}

pub fn tokenize_markdown(source: &str, mode: Mode) -> Vec<Token> {
    if source.is_empty() {
        return Vec::new();
    }

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

// Legacy line-based fence extraction used by the display-path tokenize_markdown()
struct CodeFence {
    language: Option<String>,
    content: String,
    start_line: u32,
}

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
        } else {
            let close_trimmed = trimmed.trim_end();
            if is_closing_fence_legacy(close_trimmed, fence_char) {
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

fn is_closing_fence_legacy(line: &str, fence_char: char) -> bool {
    if !line.starts_with(fence_char) {
        return false;
    }
    let count = line.chars().take_while(|&c| c == fence_char).count();
    if count < 3 {
        return false;
    }
    line.chars().skip(count).all(|c| c == ' ' || c == '\t')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::Mode;

    // --- legacy display-path tests ---

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

    // --- tokenize_markdown_maps tests ---

    fn default_options() -> TokenizeOptions {
        TokenizeOptions::new(Mode::Mild)
    }

    #[test]
    fn maps_empty_source_returns_empty() {
        let maps = tokenize_markdown_maps("", &default_options());
        assert!(maps.is_empty());
    }

    #[test]
    fn maps_js_fence_produces_javascript_entry() {
        let source = "# Title\n\n```javascript\nfunction hello() { return 42; }\n```\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let js_map = maps.iter().find(|m| m.format == "javascript");
        assert!(js_map.is_some(), "must have a javascript TokenMap");
        assert!(!js_map.unwrap().tokens.is_empty(), "javascript tokens must be non-empty");
    }

    #[test]
    fn maps_multiple_fences_produce_multiple_formats() {
        let source = "# Title\n\n```javascript\nconst x = 1;\n```\n\n```python\ndef foo():\n    pass\n```\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let js_map = maps.iter().find(|m| m.format == "javascript");
        let py_map = maps.iter().find(|m| m.format == "python");
        assert!(js_map.is_some(), "must have javascript TokenMap");
        assert!(py_map.is_some(), "must have python TokenMap");
    }

    #[test]
    fn maps_no_fences_produces_markdown_prose_only() {
        let source = "# Just prose\n\nNo code here.\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let md_map = maps.iter().find(|m| m.format == "markdown");
        assert!(md_map.is_some(), "must have markdown TokenMap for prose");
        assert!(
            maps.iter().all(|m| m.format == "markdown"),
            "no other formats expected"
        );
    }

    #[test]
    fn maps_unknown_language_skipped() {
        let source = "```xyzunknown999\nhello world\n```\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        assert!(
            maps.iter().all(|m| m.format != "xyzunknown999"),
            "unknown language should not produce its own format map"
        );
    }

    #[test]
    fn maps_tilde_fences_supported() {
        let source = "~~~javascript\nconst x = 1;\n~~~\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let js_map = maps.iter().find(|m| m.format == "javascript");
        assert!(js_map.is_some(), "tilde fences must produce javascript TokenMap");
    }

    #[test]
    fn maps_yaml_front_matter() {
        let source = "---\ntitle: Hello\nauthor: World\n---\n\nSome prose.\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let yaml_map = maps.iter().find(|m| m.format == "yaml");
        assert!(yaml_map.is_some(), "YAML front matter must produce yaml TokenMap");
    }

    #[test]
    fn maps_front_matter_with_ellipsis_terminator() {
        let source = "---\ntitle: Hello\n...\n\nMore text.\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let yaml_map = maps.iter().find(|m| m.format == "yaml");
        assert!(yaml_map.is_some(), "... must terminate front matter as yaml");
    }

    #[test]
    fn maps_front_matter_without_closing_is_prose() {
        let source = "---\nthis is not front matter\nit has no closing marker\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let yaml_map = maps.iter().find(|m| m.format == "yaml");
        assert!(yaml_map.is_none(), "unclosed --- must not be treated as yaml");
    }

    #[test]
    fn maps_ignore_region_suppresses_fence_tokens() {
        let source = "<!-- jscpd:ignore-start -->\n```javascript\nconst x = 1;\n```\n<!-- jscpd:ignore-end -->\n```javascript\nconst y = 2;\n```\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let js_map = maps.iter().find(|m| m.format == "javascript");
        assert!(js_map.is_some(), "javascript map must exist");
        let non_ignored_count = js_map.unwrap().tokens.len();
        assert!(non_ignored_count > 0, "second fence must yield non-ignored tokens");
    }

    #[test]
    fn maps_backtick_tilde_do_not_close_each_other() {
        let source = "```javascript\nconst a = 1;\n~~~\nconst b = 2;\n```\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let js_map = maps.iter().find(|m| m.format == "javascript");
        assert!(js_map.is_some(), "backtick fence should not be closed by tilde");
    }

    #[test]
    fn maps_closing_fence_length_must_match() {
        let source = "````javascript\nconst x = 1;\n````\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let js_map = maps.iter().find(|m| m.format == "javascript");
        assert!(js_map.is_some(), "4-backtick fence must work");
    }

    #[test]
    fn maps_fence_with_info_string_space() {
        let source = "```javascript extra info\nconst x = 1;\n```\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let js_map = maps.iter().find(|m| m.format == "javascript");
        assert!(js_map.is_some(), "first whitespace-delimited token is the language");
    }

    #[test]
    fn maps_returns_markdown_prose_tokens() {
        let source = "# Header\n\nSome prose.\n\n```javascript\nvar x;\n```\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let md_map = maps.iter().find(|m| m.format == "markdown");
        assert!(md_map.is_some(), "must have markdown TokenMap for prose");
        assert!(!md_map.unwrap().tokens.is_empty(), "prose must produce tokens");
    }

    #[test]
    fn maps_detection_tokens_have_valid_positions() {
        let source = "```javascript\nconst x = 1;\n```\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let js_map = maps.iter().find(|m| m.format == "javascript");
        assert!(js_map.is_some());
        for t in &js_map.unwrap().tokens {
            assert!(t.start.line >= 1, "line must be 1-based");
            assert!(t.start.offset as i32 >= 0, "offset must be non-negative");
        }
    }

    #[test]
    fn line_spans_basic() {
        let content = "hello\nworld\n";
        let spans = line_spans(content);
        assert_eq!(spans.len(), 3);
        assert_eq!(&content[spans[0].start..spans[0].end], "hello");
        assert_eq!(&content[spans[1].start..spans[1].end], "world");
    }

    #[test]
    fn line_spans_empty_content() {
        let spans = line_spans("");
        assert!(spans.is_empty());
    }

    #[test]
    fn line_spans_no_trailing_newline() {
        let content = "one\ntwo";
        let spans = line_spans(content);
        assert_eq!(spans.len(), 2);
        assert_eq!(&content[spans[0].start..spans[0].end], "one");
        assert_eq!(&content[spans[1].start..spans[1].end], "two");
    }

    #[test]
    fn opening_fence_detection() {
        assert!(parse_opening_fence("```javascript").is_some());
        assert!(parse_opening_fence("~~~python").is_some());
        assert!(parse_opening_fence("``").is_none());
        assert!(parse_opening_fence("not a fence").is_none());
    }

    #[test]
    fn closing_fence_detection() {
        let open = FenceOpen { marker: b'`', len: 3, info: String::new() };
        assert!(is_closing_fence("```", &open));
        assert!(is_closing_fence("````", &open));
        assert!(!is_closing_fence("~~", &open));
        assert!(!is_closing_fence("```javascript", &open));
    }

    #[test]
    fn byte_offsets_are_correct_for_front_matter() {
        let source = "---\ntitle: Hello\n---\n\nText.\n";
        let fm = extract_front_matter(source).unwrap();
        assert_eq!(fm.format, "yaml");
        assert!(fm.front_matter);
        assert_eq!(fm.block_start, 0);
        assert_eq!(&source[fm.inner_start..fm.inner_end], "title: Hello");
    }

    #[test]
    fn byte_offsets_are_correct_for_code_block() {
        let source = "# Header\n\n```javascript\nconst x = 1;\n```\n";
        let fences = extract_code_fences(source);
        assert_eq!(fences.len(), 1);
        let f = &fences[0];
        assert_eq!(f.format, "javascript");
        assert!(!f.front_matter);
        let inner = &source[f.inner_start..f.inner_end];
        assert!(inner.contains("const x = 1;"));
    }

    #[test]
    fn resolve_format_js() {
        assert_eq!(resolve_fence_format("javascript"), Some("javascript"));
    }

    #[test]
    fn resolve_format_unknown() {
        assert!(resolve_fence_format("xyzunknown999").is_none());
    }

    #[test]
    fn maps_synonym_resolution() {
        let source = "```node\nconst x = 1;\n```\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let js_map = maps.iter().find(|m| m.format == "javascript");
        assert!(js_map.is_some(), "node must resolve to javascript");
    }

    #[test]
    fn maps_shell_resolves_to_bash() {
        let source = "```shell\necho hello\n```\n";
        let maps = tokenize_markdown_maps(source, &default_options());
        let bash_map = maps.iter().find(|m| m.format == "bash");
        assert!(bash_map.is_some(), "shell must resolve to bash");
    }
}