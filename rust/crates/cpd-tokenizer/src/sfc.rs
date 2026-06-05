// Attribution: SFC block extraction for Vue/Svelte/Astro; inspired by jscpd-rs approach; rewritten independently.

use std::collections::BTreeMap;

use cpd_core::models::{DetectionToken, Token};

use crate::embedded::blank_ranges_preserve_newlines;
use crate::line_index::LineIndex;
use crate::markdown::{offset_detection_tokens, tokens_to_detection};
use crate::tokenizer::{Mode, TokenMap, TokenizeOptions};

#[derive(Debug, Clone)]
pub struct Block {
    pub block_format: String,
    pub content: String,
    pub start_offset: usize,
    pub start_line: u32,
}

#[allow(dead_code)]
struct SfcBlock {
    tag: String,
    block_format: String,
    block_start: usize,
    inner_start: usize,
    inner_end: usize,
    block_end: usize,
}

pub fn tokenize_sfc_maps(
    source: &str,
    file_format: &str,
    options: &TokenizeOptions,
) -> Vec<TokenMap> {
    if source.is_empty() {
        return Vec::new();
    }

    let blocks = find_sfc_blocks(source, file_format);
    if blocks.is_empty() {
        let tokens = crate::generic::tokenize_generic(source, "html");
        let detection = tokens_to_detection(tokens, options);
        return if detection.is_empty() {
            Vec::new()
        } else {
            vec![TokenMap {
                format: "html".to_string(),
                tokens: detection,
            }]
        };
    }

    let blank_ranges: Vec<[usize; 2]> = blocks
        .iter()
        .filter_map(|b| {
            if b.inner_start < b.inner_end {
                Some([b.inner_start, b.inner_end])
            } else {
                None
            }
        })
        .collect();

    let sanitized = blank_ranges_preserve_newlines(source, &blank_ranges);
    let line_index = LineIndex::new(source.as_bytes());

    let mut grouped: BTreeMap<String, Vec<DetectionToken>> = BTreeMap::new();

    let markup_tokens = crate::generic::tokenize_generic(&sanitized, "html");
    let mut markup_detection = tokens_to_detection(markup_tokens, options);
    markup_detection.retain(|t| t.range[0] < t.range[1]);
    if !markup_detection.is_empty() {
        grouped
            .entry("html".to_string())
            .or_default()
            .extend(markup_detection);
    }

    for block in &blocks {
        if block.inner_start >= block.inner_end {
            continue;
        }
        let inner = &source[block.inner_start..block.inner_end];
        let inner_start_loc = line_index.location(block.inner_start);

        let mut inner_tokens = tokenize_sfc_block_inner(&block.block_format, inner, options);
        offset_detection_tokens(&mut inner_tokens, block.inner_start, &inner_start_loc);

        grouped
            .entry(block.block_format.clone())
            .or_default()
            .extend(inner_tokens);
    }

    grouped
        .into_iter()
        .filter(|(_, tokens)| !tokens.is_empty())
        .map(|(format, tokens)| TokenMap { format, tokens })
        .collect()
}

fn tokenize_sfc_block_inner(
    format: &str,
    source: &str,
    options: &TokenizeOptions,
) -> Vec<DetectionToken> {
    let raw = match format {
        "javascript" | "typescript" | "jsx" | "tsx" => {
            crate::javascript::tokenize_js(source, format)
        }
        "vue" | "svelte" | "astro" => crate::sfc::tokenize_sfc(source, format, options.mode),
        "markdown" | "md" => crate::generic::tokenize_generic(source, format),
        _ => crate::generic::tokenize_generic(source, format),
    };
    tokens_to_detection(raw, options)
}

fn find_sfc_blocks(source: &str, file_format: &str) -> Vec<SfcBlock> {
    let source_lower = source.to_ascii_lowercase();
    let tag_names: &[&str] = match file_format {
        "svelte" | "astro" => &["script", "style"],
        _ => &["template", "script", "style"],
    };

    let mut blocks = Vec::new();

    if file_format == "astro" {
        if let Some(fm) = astro_frontmatter_block(source) {
            blocks.push(fm);
        }
    }

    for tag in tag_names {
        let mut search_from = 0usize;
        while let Some(block) = find_sfc_tag_block(source, &source_lower, tag, search_from) {
            search_from = block.block_end;
            blocks.push(block);
        }
    }

    blocks.sort_by_key(|b| b.block_start);
    blocks
}

fn find_sfc_tag_block(
    source: &str,
    source_lower: &str,
    tag: &str,
    from: usize,
) -> Option<SfcBlock> {
    let open_needle = format!("<{}", tag);
    let close_needle = format!("</{}>", tag);

    let open_start = source_lower[from..].find(&open_needle)? + from;
    let after_tag_name = open_start + 1 + tag.len();
    if source_lower
        .as_bytes()
        .get(after_tag_name)
        .is_some_and(|b| b.is_ascii_alphabetic())
    {
        return None;
    }
    let tag_end = source_lower[open_start..].find('>')? + open_start + 1;
    let close_start = source_lower[tag_end..].find(&close_needle)? + tag_end;

    let attrs = &source[open_start + 1 + tag.len()..tag_end];
    let inner_start = tag_end;
    let inner_end = close_start;
    let block_end = source_lower[close_start..]
        .find('>')
        .map(|i| close_start + i + 1)
        .unwrap_or(close_start + close_needle.len());
    let block_end = block_end.min(source.len());

    let block_format = detect_sfc_block_format(attrs, tag);

    Some(SfcBlock {
        tag: tag.to_string(),
        block_format,
        block_start: open_start,
        inner_start,
        inner_end: inner_end.max(inner_start),
        block_end,
    })
}

fn detect_sfc_block_format(attrs: &str, tag: &str) -> String {
    let lang = extract_lang_attr_value(attrs);
    match tag {
        "script" => match lang.as_deref() {
            Some("ts" | "typescript") => "typescript".to_string(),
            Some("js" | "javascript") => "javascript".to_string(),
            Some(other) => {
                if crate::formats::get_format_by_extension(other).is_some()
                    || crate::formats::SUPPORTED_FORMATS
                        .iter()
                        .any(|e| e.name == other)
                {
                    other.to_string()
                } else {
                    "javascript".to_string()
                }
            }
            None => "javascript".to_string(),
        },
        "style" => match lang.as_deref() {
            Some("scss" | "sass") => "scss".to_string(),
            Some("less") => "less".to_string(),
            _ => "css".to_string(),
        },
        "template" => match lang.as_deref() {
            Some(v) if v == "pug" || v == "jade" => "pug".to_string(),
            _ => "html".to_string(),
        },
        _ => "html".to_string(),
    }
}

fn astro_frontmatter_block(source: &str) -> Option<SfcBlock> {
    if !(source.starts_with("---\n") || source.starts_with("---\r\n")) {
        return None;
    }
    let lines = crate::markdown::line_spans(source);
    let close_idx = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, span)| source[span.start..span.end].trim() == "---")
        .map(|(idx, _)| idx)?;
    let inner_start = lines.get(1)?.start;
    let inner_end = source[..lines[close_idx].start]
        .strip_suffix('\n')
        .map(|prefix: &str| prefix.len())
        .unwrap_or(lines[close_idx].start);
    let block_end = lines[close_idx].next_start.min(source.len());
    Some(SfcBlock {
        tag: "script".to_string(),
        block_format: "typescript".to_string(),
        block_start: 0,
        inner_start,
        inner_end: inner_end.max(inner_start),
        block_end,
    })
}

fn extract_lang_attr_value(attrs: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let lang_pos = lower.find("lang=")?;
    let rest = &attrs[lang_pos + 5..];
    let quote = if rest.starts_with('"') {
        '"'
    } else if rest.starts_with('\'') {
        '\''
    } else {
        return None;
    };
    let value_start = 1;
    let value_end = rest[value_start..].find(quote)? + value_start;
    Some(rest[value_start..value_end].to_ascii_lowercase())
}

/// Extract blocks from a Vue/Svelte/Astro file (display path).
pub fn extract_blocks(source: &str, file_format: &str) -> Vec<Block> {
    let source_lower = source.to_ascii_lowercase();
    let tag_names: &[&str] = match file_format {
        "svelte" | "astro" => &["script", "style"],
        _ => &["template", "script", "style"],
    };

    let mut blocks = Vec::new();
    for tag in tag_names {
        let mut search_from = 0;
        while let Some((block, next_from)) =
            find_display_block(source, &source_lower, tag, search_from)
        {
            search_from = next_from;
            blocks.push(block);
        }
    }
    blocks.sort_by_key(|b: &Block| b.start_offset);
    blocks
}

fn find_display_block(
    source: &str,
    source_lower: &str,
    tag: &str,
    from: usize,
) -> Option<(Block, usize)> {
    let open_needle = format!("<{}", tag);
    let close_needle = format!("</{}>", tag);

    let open_start = source_lower[from..].find(&open_needle)? + from;
    let after_tag_name = open_start + 1 + tag.len();
    if source_lower
        .as_bytes()
        .get(after_tag_name)
        .is_some_and(|b| b.is_ascii_alphabetic())
    {
        return None;
    }
    let tag_end = source_lower[open_start..].find('>')? + open_start + 1;
    let close_start = source_lower[tag_end..].find(&close_needle)? + tag_end;

    let attrs = &source[open_start + 1 + tag.len()..tag_end];
    let content = source[tag_end..close_start].to_string();
    let content_len = content.len();
    let start_line = source[..tag_end].lines().count() as u32 + 1;
    let block_format = detect_display_block_format(attrs, tag);

    Some((
        Block {
            block_format,
            content,
            start_offset: tag_end,
            start_line,
        },
        tag_end + content_len,
    ))
}

fn detect_display_block_format(attrs: &str, tag: &str) -> String {
    let lang = extract_lang_attr_value(attrs);
    match tag {
        "script" => match lang.as_deref() {
            Some("ts" | "typescript") => "typescript".to_string(),
            Some("js" | "javascript") => "javascript".to_string(),
            _ => "javascript".to_string(),
        },
        "style" => match lang.as_deref() {
            Some("scss" | "sass") => "scss".to_string(),
            Some("less") => "less".to_string(),
            _ => "css".to_string(),
        },
        "template" => "html".to_string(),
        _ => "html".to_string(),
    }
}

pub fn tokenize_sfc(source: &str, file_format: &str, mode: Mode) -> Vec<Token> {
    let blocks = extract_blocks(source, file_format);
    let mut all_tokens = Vec::new();

    for block in &blocks {
        let mut block_tokens =
            crate::tokenizer::tokenize(&block.block_format, &block.content, mode);
        let line_offset = block.start_line.saturating_sub(1);
        for token in &mut block_tokens {
            token.start.line += line_offset;
            token.end.line += line_offset;
        }
        all_tokens.extend(block_tokens);
    }

    all_tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    const VUE_FILE: &str = r#"<template>
  <div>Hello</div>
</template>

<script>
export default { name: 'Foo' }
</script>

<style>
.foo { color: red; }
</style>
"#;

    const VUE_TS_FILE: &str = r#"<template>
  <div>Hello</div>
</template>

<script lang="ts">
const x: number = 5;
</script>

<style lang="scss">
.foo { color: red; }
</style>
"#;

    #[test]
    fn vue_file_extracts_three_blocks() {
        let blocks = extract_blocks(VUE_FILE, "vue");
        assert_eq!(blocks.len(), 3, "must find template, script, style blocks");
    }

    #[test]
    fn script_block_default_format_is_javascript() {
        let blocks = extract_blocks(VUE_FILE, "vue");
        let script = blocks.iter().find(|b| b.block_format == "javascript");
        assert!(script.is_some(), "plain <script> must be javascript format");
    }

    #[test]
    fn script_lang_ts_produces_typescript_format() {
        let blocks = extract_blocks(VUE_TS_FILE, "vue");
        let ts_block = blocks.iter().find(|b| b.block_format == "typescript");
        assert!(
            ts_block.is_some(),
            "<script lang=\"ts\"> must produce typescript format"
        );
    }

    #[test]
    fn unknown_lang_does_not_panic() {
        let source = "<script lang=\"unknownlang123\">\nconst x = 1;\n</script>\n";
        let result = std::panic::catch_unwind(|| extract_blocks(source, "vue"));
        assert!(result.is_ok(), "unknown lang must not panic");
    }

    #[test]
    fn no_blocks_returns_empty() {
        let source = "just plain text no tags";
        let blocks = extract_blocks(source, "vue");
        assert!(blocks.is_empty());
    }

    #[test]
    fn start_offset_is_after_opening_tag() {
        let blocks = extract_blocks(VUE_FILE, "vue");
        for block in &blocks {
            assert!(block.start_offset > 0);
        }
    }

    #[test]
    fn vue_sfc_maps_produces_multiple_formats() {
        let options = TokenizeOptions::new(Mode::Mild);
        let maps = tokenize_sfc_maps(VUE_FILE, "vue", &options);
        let formats: Vec<&str> = maps.iter().map(|m| m.format.as_str()).collect();
        assert!(formats.contains(&"javascript"), "must have javascript map");
        assert!(formats.contains(&"css"), "must have css map");
        assert!(formats.contains(&"html"), "must have html map");
    }

    #[test]
    fn vue_ts_maps_produces_typescript() {
        let options = TokenizeOptions::new(Mode::Mild);
        let maps = tokenize_sfc_maps(VUE_TS_FILE, "vue", &options);
        let formats: Vec<&str> = maps.iter().map(|m| m.format.as_str()).collect();
        assert!(formats.contains(&"typescript"), "must have typescript map");
        assert!(formats.contains(&"scss"), "must have scss map");
    }

    #[test]
    fn empty_sfc_returns_empty() {
        let options = TokenizeOptions::new(Mode::Mild);
        let maps = tokenize_sfc_maps("", "vue", &options);
        assert!(maps.is_empty());
    }

    #[test]
    fn svelte_sfc_maps_produces_multiple_formats() {
        let source = r#"<script>
  let count = 0;
</script>

<style>
  .count { color: blue; }
</style>
"#;
        let options = TokenizeOptions::new(Mode::Mild);
        let maps = tokenize_sfc_maps(source, "svelte", &options);
        let formats: Vec<&str> = maps.iter().map(|m| m.format.as_str()).collect();
        assert!(
            formats.contains(&"javascript"),
            "svelte must have javascript map"
        );
        assert!(formats.contains(&"css"), "svelte must have css map");
        assert!(
            formats.contains(&"html"),
            "svelte must have html markup map"
        );
    }
}
