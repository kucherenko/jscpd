//! Positions: jscpd keeps byte offsets into a file, LSP wants a line from 0
//! and a column in UTF-16 code units, or in bytes when the client offers
//! UTF-8. A [`LineIndex`] turns one into the other from the text itself.

use lsp_types::{Position, Range, Uri};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// How columns are counted, as agreed in `initialize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf16,
}

/// The start of every line of a text, for turning byte offsets into
/// positions and back.
pub struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            text.bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'\n')
                .map(|(i, _)| i + 1),
        );
        Self { starts }
    }

    /// The position of byte `offset` of `text`, the text the index was built
    /// from. An offset past the end, or inside a character, clamps.
    pub fn position(&self, text: &str, offset: usize, encoding: Encoding) -> Position {
        let offset = floor_char_boundary(text, offset.min(text.len()));
        let line = self.starts.partition_point(|&start| start <= offset) - 1;
        let start = self.starts[line];
        let column = match encoding {
            Encoding::Utf8 => offset - start,
            Encoding::Utf16 => text[start..offset].encode_utf16().count(),
        };
        Position::new(line as u32, column as u32)
    }

    /// The range from byte `start` to byte `end` of `text`.
    pub fn range(&self, text: &str, start: usize, end: usize, encoding: Encoding) -> Range {
        Range::new(
            self.position(text, start, encoding),
            self.position(text, end, encoding),
        )
    }

    /// The bytes of line `line` (from 0), without its line break.
    pub fn line<'t>(&self, text: &'t str, line: usize) -> &'t str {
        let Some(&start) = self.starts.get(line) else {
            return "";
        };
        let end = self
            .starts
            .get(line + 1)
            .map_or(text.len(), |next| next - 1);
        text[start..end].trim_end_matches('\r')
    }

    /// The byte where line `line` (from 0) starts.
    pub fn line_start(&self, line: usize) -> Option<usize> {
        self.starts.get(line).copied()
    }

    pub fn line_count(&self) -> usize {
        self.starts.len()
    }
}

fn floor_char_boundary(text: &str, mut offset: usize) -> usize {
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// The path of a `file:` URI.
pub fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    url::Url::parse(uri.as_str()).ok()?.to_file_path().ok()
}

/// The `file:` URI of an absolute path.
pub fn path_to_uri(path: &Path) -> Option<Uri> {
    let url = url::Url::from_file_path(path).ok()?;
    Uri::from_str(url.as_str()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_columns_count_code_units_and_utf8_columns_count_bytes() {
        // "é" is 2 bytes and 1 UTF-16 unit; "𝄞" is 4 bytes and 2 units.
        let text = "ab\né𝄞x\nlast";
        let index = LineIndex::new(text);
        let x = text.find('x').unwrap();
        assert_eq!(
            index.position(text, x, Encoding::Utf16),
            Position::new(1, 3)
        );
        assert_eq!(index.position(text, x, Encoding::Utf8), Position::new(1, 6));
        assert_eq!(
            index.range(text, text.find('é').unwrap(), x, Encoding::Utf16),
            Range::new(Position::new(1, 0), Position::new(1, 3))
        );
    }

    #[test]
    fn an_offset_inside_a_character_or_past_the_end_clamps() {
        let text = "a𝄞\nb";
        let index = LineIndex::new(text);
        // Bytes 2..4 are inside the clef: the position is before it.
        for inside in 2..5 {
            assert_eq!(
                index.position(text, inside, Encoding::Utf16),
                Position::new(0, 1),
                "byte {inside}"
            );
        }
        assert_eq!(
            index.position(text, 999, Encoding::Utf16),
            Position::new(1, 1)
        );
    }

    #[test]
    fn a_line_comes_without_its_line_break() {
        let text = "ab\r\ncd\n";
        let index = LineIndex::new(text);
        assert_eq!(index.line_count(), 3);
        assert_eq!(index.line(text, 0), "ab");
        assert_eq!(index.line(text, 1), "cd");
        assert_eq!(index.line(text, 2), "");
        assert_eq!(index.line(text, 9), "", "a line past the end is empty");
        assert_eq!(index.line_start(1), Some(4));
        assert_eq!(index.line_start(9), None);
        // The end of the text of a CRLF line is before its `\r`.
        assert_eq!(
            index.position(text, 2, Encoding::Utf16),
            Position::new(0, 2)
        );
    }

    #[test]
    fn paths_and_uris_round_trip() {
        let path = std::env::temp_dir().join("a b").join("c.ts");
        let uri = path_to_uri(&path).unwrap();
        assert!(uri.as_str().starts_with("file://"), "{}", uri.as_str());
        assert!(uri.as_str().contains("a%20b"), "{}", uri.as_str());
        assert_eq!(uri_to_path(&uri).unwrap(), path);
    }

    #[test]
    fn a_uri_that_is_not_a_file_has_no_path() {
        let uri = Uri::from_str("untitled:Untitled-1").unwrap();
        assert_eq!(uri_to_path(&uri), None);
        assert_eq!(path_to_uri(Path::new("relative/a.js")), None);
    }
}
