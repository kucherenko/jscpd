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
