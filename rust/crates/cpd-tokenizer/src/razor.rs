use std::collections::BTreeMap;

use cpd_core::models::{DetectionToken, Token};

use crate::embedded::blank_ranges_preserve_newlines;
use crate::line_index::LineIndex;
use crate::markdown::{offset_detection_tokens, tokens_to_detection};
use crate::tokenizer::{Mode, TokenMap, TokenizeOptions, tokenize_format_to_detection};

#[derive(Debug, Clone)]
struct RazorBlock {
    content: String,
    start_offset: usize,
    start_line: u32,
}

/// Extract Razor code blocks (@-expressions and C# code).
/// Strategy: Mark regions as HTML or code based on @ prefixes and braces.
fn extract_razor_blocks(source: &str) -> Vec<RazorBlock> {
    let mut blocks = Vec::new();
    let mut current_block: Option<RazorBlock> = None;
    let mut in_code = false;
    let mut brace_depth = 0;
    let bytes = source.as_bytes();
    let mut offset = 0usize;
    let mut line = 1u32;

    while offset < bytes.len() {
        let ch = bytes[offset] as char;

        // Track line numbers for accurate location reporting
        if ch == '\n' {
            line += 1;
        }

        // Detect @ entry into code
        if !in_code && ch == '@' && offset + 1 < bytes.len() {
            let next_byte = bytes[offset + 1];
            let next_ch = next_byte as char;

            // Skip escaped @@
            if next_ch == '@' {
                offset += 2;
                continue;
            }

            // Start block if @ is followed by identifier char, { or (
            if next_ch.is_alphabetic() || next_ch == '_' || next_ch == '{' || next_ch == '(' {
                current_block = Some(RazorBlock {
                    content: String::new(),
                    start_offset: offset,
                    start_line: line,
                });
                in_code = true;
                brace_depth = 0;
            }
        }

        // Collect code content
        if in_code {
            let is_boundary = brace_depth == 0
                && (ch.is_whitespace() || matches!(ch, '[' | ']' | '<' | '>' | '&' | ';' | ','));
            if is_boundary {
                if let Some(block) = current_block.take() {
                    if !block.content.is_empty() {
                        blocks.push(block);
                    }
                }
                in_code = false;
                offset += 1;
                continue;
            }

            if let Some(ref mut block) = current_block {
                block.content.push(ch);
            }

            match ch {
                // Track braces for block boundaries
                '{' => {
                    brace_depth += 1;
                }
                '}' => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                        // Block ended, flush it
                        if brace_depth == 0 {
                            if let Some(block) = current_block.take() {
                                blocks.push(block);
                            }
                            in_code = false;
                        }
                    }
                }
                // Single-line expressions end at delimiters (if not in braces)
                _ if brace_depth == 0 && (ch.is_whitespace() || matches!(ch, '[' | ']' | '<' | '>' | '&' | ';' | ',')) => {
                    if let Some(block) = current_block.take() {
                        // Trim the last char since it belongs to the delimiter
                        if !block.content.is_empty() {
                            blocks.push(block);
                        }
                    }
                    in_code = false;
                }
                _ => {}
            }
        }

        offset += 1;
    }

    // Flush any remaining open block
    if let Some(block) = current_block.take() {
        blocks.push(block);
    }

    blocks
}

/// Tokenize Razor for the detection path (returns TokenMap per format).
/// This is the hot path used by clone detection.
pub fn tokenize_razor_maps(
    source: &str,
    options: &TokenizeOptions,
) -> Vec<TokenMap> {
    if source.is_empty() {
        return Vec::new();
    }

    let blocks = extract_razor_blocks(source);

    if blocks.is_empty() {
        // Pure HTML, no code blocks or @ directives
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
        .map(|b| [b.start_offset, b.start_offset + b.content.len()])
        .collect();

    // Sanitize code regions while preserving line structure
    let sanitized = blank_ranges_preserve_newlines(source, &blank_ranges);
    let line_index = LineIndex::new(source.as_bytes());

    let mut grouped: BTreeMap<String, Vec<DetectionToken>> = BTreeMap::new();

    // Tokenize HTML skeleton (with code blanked)
    let html_tokens = crate::generic::tokenize_generic(&sanitized, "html");
    let mut html_detection = tokens_to_detection(html_tokens, options);
    html_detection.retain(|t| t.range[0] < t.range[1]);
    if !html_detection.is_empty() {
        grouped
            .entry("html".to_string())
            .or_default()
            .extend(html_detection);
    }

    // Tokenize code blocks
    for block in &blocks {
        let inner_tokens = tokenize_format_to_detection(
            "csharp",
            &block.content,
            options,
        );

        if !inner_tokens.is_empty() {
            let inner_start_loc = line_index.location(block.start_offset);
            let mut offset_tokens = inner_tokens;
            offset_detection_tokens(&mut offset_tokens, block.start_offset, &inner_start_loc);

            grouped
                .entry("csharp".to_string())
                .or_default()
                .extend(offset_tokens);
        }
    }

    grouped
        .into_iter()
        .filter(|(_, tokens)| !tokens.is_empty())
        .map(|(format, tokens)| TokenMap { format, tokens })
        .collect()
}

pub fn tokenize_razor(source: &str, mode: Mode) -> Vec<Token> {
    let blocks = extract_razor_blocks(source);
    let mut all_tokens = Vec::new();

    for block in &blocks {
        let mut block_tokens = crate::tokenizer::tokenize("csharp", &block.content, mode);

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

    const RAZOR_CSHARP: &str = r#"@page "/products"
@using MyApp.Models

<div>
    <h1>@Model.Title</h1>
    @foreach (var item in Model.Items) {
        <p>@item.Name - @item.Price</p>
    }
    @if (Model.IsSpecial) {
        <span class="badge">Special Offer</span>
    }

    <span>@(Model.SpecialDescription)</span>
</div>"#;

    const RAZOR_WITH_EXPRESSION: &str = r#"<div>
    Current time: @DateTime.Now
    User: @User.Identity.Name
</div>"#;

    const PURE_HTML: &str = r#"<div>
    <h1>No Code Here</h1>
    <p>Just HTML</p>
</div>"#;

    #[test]
    fn razor_file_extracts_code_blocks() {
        let blocks = extract_razor_blocks(RAZOR_CSHARP);
        assert!(!blocks.is_empty(), "must extract code blocks");
    }

    #[test]
    fn razor_detects_foreach_block() {
        let blocks = extract_razor_blocks(RAZOR_CSHARP);
        let foreach_block = blocks.iter().find(|b| b.content.contains("foreach"));
        assert!(foreach_block.is_some(), "must detect @foreach block");
    }

    #[test]
    fn razor_detects_if_block() {
        let blocks = extract_razor_blocks(RAZOR_CSHARP);
        let if_block = blocks.iter().find(|b| b.content.contains("if"));
        assert!(if_block.is_some(), "must detect @if block");
    }

    #[test]
    fn razor_detects_single_expression() {
        let blocks = extract_razor_blocks(RAZOR_WITH_EXPRESSION);
        assert!(!blocks.is_empty(), "must detect single-line expressions");
    }

    #[test]
    fn pure_html_returns_no_blocks() {
        let blocks = extract_razor_blocks(PURE_HTML);
        assert!(blocks.is_empty(), "pure HTML must produce no code blocks");
    }

    #[test]
    fn razor_maps_produces_html_and_csharp() {
        let options = TokenizeOptions::new(crate::tokenizer::Mode::Mild);
        let maps = tokenize_razor_maps(RAZOR_CSHARP, &options);
        let formats: Vec<&str> = maps.iter().map(|m| m.format.as_str()).collect();
        assert!(formats.contains(&"html"), "must have html map");
        assert!(formats.contains(&"csharp"), "must have csharp map");
    }

    #[test]
    fn razor_maps_pure_html_returns_html_only() {
        let options = TokenizeOptions::new(crate::tokenizer::Mode::Mild);
        let maps = tokenize_razor_maps(PURE_HTML, &options);
        let formats: Vec<&str> = maps.iter().map(|m| m.format.as_str()).collect();
        assert!(formats.contains(&"html"), "must have html map for pure HTML");
    }

    #[test]
    fn tokenize_razor_does_not_panic() {
        let result = std::panic::catch_unwind(|| {
            tokenize_razor(RAZOR_CSHARP, Mode::Mild)
        });
        assert!(result.is_ok(), "tokenize_razor must not panic");
    }

    #[test]
    fn escaped_at_sign_not_treated_as_code() {
        let source = "Price: @@50.00";
        let blocks = extract_razor_blocks(source);
        assert!(blocks.is_empty(), "@@ must not start a code block");
    }
}
