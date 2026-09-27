//! Text helpers for article titles and previews.

const MAX_SLUG_LEN: usize = 80;

/// `"Hello, World! 2026"` becomes `"hello-world-2026"`.
pub fn slugify(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    let mut pending_dash = false;
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(c);
            pending_dash = false;
        } else {
            pending_dash = true;
        }
    }
    if slug.len() > MAX_SLUG_LEN {
        slug.truncate(MAX_SLUG_LEN);
        while slug.ends_with('-') {
            slug.pop();
        }
    }
    slug
}

/// The first `max_words` words of `body`, with an ellipsis when cut.
pub fn excerpt(body: &str, max_words: usize) -> String {
    let words: Vec<&str> = body.split_whitespace().collect();
    if words.len() <= max_words {
        return words.join(" ");
    }
    let mut cut = words[..max_words].join(" ");
    while cut.ends_with(|c: char| matches!(c, ',' | ';' | ':' | '.' | '-')) {
        cut.pop();
    }
    cut.push('…');
    cut
}
