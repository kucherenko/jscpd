// Attribution: SFC block extraction for Vue/Svelte/Astro; inspired by jscpd-rs approach; rewritten independently.

use cpd_core::models::Token;

#[derive(Debug, Clone)]
pub struct Block {
    pub block_format: String,
    pub content: String,
    pub start_offset: usize,
    pub start_line: u32,
}

/// Extract blocks from a Vue/Svelte/Astro file.
/// Returns one Block per top-level `<script>`, `<style>`, or `<template>` tag found.
pub fn extract_blocks(source: &str, file_format: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let parent_format = sfc_parent_format(file_format);

    let tag_names = ["script", "style", "template"];

    for tag in &tag_names {
        let mut search_from = 0;
        while let Some(block) = find_block(source, tag, search_from, parent_format) {
            search_from = block.start_offset + block.content.len();
            blocks.push(block);
        }
    }

    // Sort by start_offset so blocks appear in document order
    blocks.sort_by_key(|b| b.start_offset);
    blocks
}

/// Find a single <tag ...>content</tag> block starting from `from` offset.
fn find_block(source: &str, tag: &str, from: usize, parent_format: &str) -> Option<Block> {
    let open_tag = format!("<{}", tag);
    let close_tag = format!("</{}>", tag);

    let open_start = source[from..].find(&open_tag)? + from;
    let tag_end = source[open_start..].find('>')? + open_start + 1;
    let close_start = source[tag_end..].find(&close_tag)? + tag_end;

    let attrs = &source[open_start..tag_end];
    let content = &source[tag_end..close_start];
    let start_line = source[..tag_end].lines().count() as u32 + 1;

    let block_format = detect_block_format(attrs, tag, parent_format);

    Some(Block {
        block_format,
        content: content.to_string(),
        start_offset: tag_end,
        start_line,
    })
}

/// Detect the language format of a block from its `lang="..."` attribute.
fn detect_block_format(attrs: &str, tag: &str, parent_format: &str) -> String {
    if let Some(lang) = extract_lang_attr(attrs) {
        match lang.as_str() {
            "ts" | "typescript" => "typescript".to_string(),
            "js" | "javascript" => "javascript".to_string(),
            "scss" => "scss".to_string(),
            "sass" => "sass".to_string(),
            "less" => "less".to_string(),
            "css" => "css".to_string(),
            "pug" | "jade" => "pug".to_string(),
            other => {
                if crate::formats::get_format_by_extension(other).is_some() {
                    other.to_string()
                } else {
                    default_tag_format(tag, parent_format)
                }
            }
        }
    } else {
        default_tag_format(tag, parent_format)
    }
}

fn default_tag_format(tag: &str, parent_format: &str) -> String {
    match tag {
        "script" => "javascript".to_string(),
        "style" => "css".to_string(),
        "template" => "html".to_string(),
        _ => parent_format.to_string(),
    }
}

fn extract_lang_attr(attrs: &str) -> Option<String> {
    let lang_pos = attrs.find("lang=")?;
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
    Some(rest[value_start..value_end].to_string())
}

fn sfc_parent_format(file_format: &str) -> &str {
    match file_format {
        "vue" | "svelte" | "astro" => "html",
        _ => "html",
    }
}

/// Tokenize an SFC file: extract blocks, tokenize each with appropriate sub-tokenizer,
/// adjust token line numbers by block start offset, return flattened Vec<Token>.
pub fn tokenize_sfc(source: &str, file_format: &str, mode: crate::tokenizer::Mode) -> Vec<Token> {
    let blocks = extract_blocks(source, file_format);
    let mut all_tokens = Vec::new();

    for block in &blocks {
        let mut block_tokens = crate::tokenizer::tokenize(&block.block_format, &block.content, mode);
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
        assert!(ts_block.is_some(), "<script lang=\"ts\"> must produce typescript format");
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
}
