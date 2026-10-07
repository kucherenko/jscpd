use cpd_core::models::Token;

use crate::markdown::tokens_to_detection;
use crate::tokenizer::{Mode, TokenMap, TokenizeOptions};

/// Detection tokens for a container file (Razor, Vue, Svelte, Astro) that
/// holds no embedded blocks: the whole source is plain HTML, one map or none.
pub(crate) fn html_only_maps(source: &str, options: &TokenizeOptions) -> Vec<TokenMap> {
    let tokens = crate::generic::tokenize_generic(source, "html");
    let detection = tokens_to_detection(tokens, options);
    if detection.is_empty() {
        Vec::new()
    } else {
        vec![TokenMap {
            format: "html".to_string(),
            tokens: detection,
        }]
    }
}

/// Display-path tokens for embedded blocks: each `(format, content,
/// start_offset)` block, the part of `source` from that byte on, is
/// tokenized as its own format, and its tokens are placed in `source`:
/// lines, columns and byte offsets, so `--ignore-pattern` ranges of the
/// host apply to them.
pub(crate) fn tokenize_blocks_shifted<'a>(
    source: &str,
    blocks: impl IntoIterator<Item = (&'a str, &'a str, usize)>,
    mode: Mode,
) -> Vec<Token> {
    let host = crate::line_index::LineIndex::new(source.as_bytes());
    let mut all_tokens = Vec::new();
    for (format, content, start_offset) in blocks {
        let mut block_tokens = crate::tokenizer::tokenize(format, content, mode);
        for token in &mut block_tokens {
            token.start = host.location(start_offset + token.start.offset as usize);
            token.end = host.location(start_offset + token.end.offset as usize);
        }
        all_tokens.extend(block_tokens);
    }
    all_tokens
}

pub fn blank_ranges_preserve_newlines(source: &str, ranges: &[[usize; 2]]) -> String {
    let mut src_bytes = source.as_bytes().to_vec();
    let mut sorted: Vec<[usize; 2]> = ranges.to_vec();
    sorted.sort_by_key(|r| r[0]);
    for i in 1..sorted.len() {
        if sorted[i][0] < sorted[i - 1][1] {
            panic!(
                "overlapping ranges detected: [{}, {}) overlaps [{}, {})",
                sorted[i - 1][0],
                sorted[i - 1][1],
                sorted[i][0],
                sorted[i][1],
            );
        }
    }
    sorted.sort_by(|a, b| b[0].cmp(&a[0]));
    for &[start, end] in &sorted {
        for byte in &mut src_bytes[start..end] {
            match byte {
                b'\n' => {}
                _ => *byte = b' ',
            }
        }
    }
    String::from_utf8(src_bytes).expect("blanking preserves UTF-8 validity")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_ranges_returns_source_unchanged() {
        let s = "hello\nworld";
        let result = blank_ranges_preserve_newlines(s, &[]);
        assert_eq!(result, s);
    }

    #[test]
    fn single_range_blanks_non_newlines() {
        let s = "ab\ncd";
        let result = blank_ranges_preserve_newlines(s, &[[0, 5]]);
        assert_eq!(result, "  \n  ");
    }

    #[test]
    #[should_panic(expected = "overlapping ranges")]
    fn overlapping_ranges_panics() {
        let s = "hello";
        let _ = blank_ranges_preserve_newlines(s, &[[0, 3], [2, 5]]);
    }

    #[test]
    fn multiple_ranges_all_blanked() {
        let s = "foo\nbar\nbaz";
        let result = blank_ranges_preserve_newlines(s, &[[0, 3], [4, 7]]);
        assert_eq!(result, "   \n   \nbaz");
    }

    #[test]
    fn right_to_left_no_index_drift() {
        let s = "abcdefghij";
        let result = blank_ranges_preserve_newlines(s, &[[0, 3], [6, 9]]);
        assert_eq!(result, "   def   j");
    }
}
