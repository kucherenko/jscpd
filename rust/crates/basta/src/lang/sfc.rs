//! Single-file components: Vue, Svelte and Astro.
//!
//! These are not new languages. A `.vue`, `.svelte` or `.astro` file is a
//! JavaScript or TypeScript module wrapped in markup, so the work here is to
//! hand the existing oxc analyzer the script, and to account for the markup —
//! which is where a component's imports are actually used.
//!
//! Two decisions make that work without a second parser.
//!
//! **The script is masked, not extracted.** Every byte that is not script is
//! overwritten with a space, line breaks kept, so the buffer oxc parses has
//! exactly the length and the line structure of the file on disk. Every span
//! oxc reports is therefore already a position in the real file, and no
//! offset arithmetic exists to drift.
//!
//! **The markup is scanned for names.** `import Foo from './Foo.vue'` is used
//! by `<Foo />` and by nothing else. Without the template, every component
//! import in a Vue, Svelte or Astro project reads as an unused import, and
//! every module only a component imports reads as an unused file — the
//! analyzer would not merely miss findings, it would invent them. The scanner
//! is deliberately generous: it states that a name is read somewhere in the
//! markup and leaves it to the graph to decide whether any declaration
//! answers to it.

use super::{outside_strings, skip_while};
use crate::model::{ModuleId, Reference, ReferenceKind};
use cpd_tokenizer::line_index::LineIndex;
use oxc_span::SourceType;

/// jscpd formats whose files are markup wrapped around a script.
const SFC_FORMATS: &[&str] = &["vue", "svelte", "astro"];

/// True for a format [`split`] can take apart.
pub fn is_sfc(format: &str) -> bool {
    SFC_FORMATS.contains(&format)
}

/// A single-file component, taken apart.
pub struct Sfc {
    /// The file with every non-script byte blanked. Same length, same lines.
    pub script: String,
    /// The `lang` of the first script block that declared one.
    lang: Option<String>,
    /// Byte ranges of markup, in source order.
    templates: Vec<(usize, usize)>,
}

impl Sfc {
    /// The oxc source type for the script that was found.
    ///
    /// TypeScript is the default rather than JavaScript because it parses
    /// both, and a `<script>` with no `lang` is far more often plain JS than
    /// it is JSX — where `<` has to mean an element instead of a type
    /// assertion, the block says so.
    pub fn source_type(&self) -> SourceType {
        match self.lang.as_deref() {
            Some("tsx") | Some("jsx") => SourceType::tsx(),
            _ => SourceType::ts(),
        }
        .with_module(true)
    }

    /// Every module the markup imports outright.
    ///
    /// `{#await import('./Heavy.svelte')}` is an ordinary import that happens
    /// to live in the markup. The script buffer has the markup blanked, so
    /// oxc never sees it, and without this the module it names reads as dead.
    pub fn template_imports(&self, source: &str) -> Vec<(String, usize)> {
        let mut out = Vec::new();
        for &(start, end) in &self.templates {
            let Some(text) = source.get(start..end) else {
                continue;
            };
            scan_dynamic_imports(text, start, &mut out);
        }
        out
    }

    /// Every name the markup reads, as references at module top level.
    ///
    /// `source` must be the original file: the ranges index into it, and the
    /// script buffer has the markup blanked out.
    pub fn template_references(
        &self,
        source: &str,
        module: ModuleId,
        lines: &LineIndex,
    ) -> Vec<Reference> {
        let mut out = Vec::new();
        for &(start, end) in &self.templates {
            let Some(text) = source.get(start..end) else {
                continue;
            };
            scan_markup(text, start, &mut |name, kind, at| {
                out.push(Reference {
                    module,
                    name,
                    kind,
                    // The markup is the component's body: it runs whenever the
                    // component does, which is what module top level means here.
                    from: None,
                    at: lines.location(at),
                });
            });
        }
        out
    }
}

/// Take a component apart, or `None` when the format is not one of these or
/// the masked buffer would not be valid UTF-8.
pub fn split(source: &str, format: &str) -> Option<Sfc> {
    if !is_sfc(format) {
        return None;
    }
    let bytes = source.as_bytes();
    let mut mask = Blanked::new(bytes);
    let mut templates: Vec<(usize, usize)> = Vec::new();
    let mut lang: Option<String> = None;
    let mut cursor = 0usize;

    // Astro's script is the frontmatter, and it is only frontmatter when the
    // file opens with the fence.
    if format == "astro"
        && let Some(close) = astro_frontmatter(bytes)
    {
        mask.blank(0, 3);
        mask.blank(close, close + 3);
        lang = Some("ts".to_string());
        cursor = (close + 3).min(bytes.len());
    }

    // Everything else is a run of `<script>` and `<style>` blocks with markup
    // between them.
    while let Some((block, is_script)) = next_block(bytes, cursor) {
        if block.start > cursor {
            templates.push((cursor, block.start));
        }
        // An Astro `<script>` is a separate client module with its own
        // imports. Parsing it into the frontmatter's scope would redeclare
        // names, so it is dropped rather than merged; its edges are the price.
        if is_script && format != "astro" {
            mask.blank(block.start, block.body.0);
            mask.blank(block.body.1, block.end);
            if lang.is_none() {
                lang = attribute_value(&bytes[block.attrs.0..block.attrs.1], "lang");
            }
        } else {
            mask.blank(block.start, block.end);
        }
        cursor = block.end;
    }
    if cursor < bytes.len() {
        templates.push((cursor, bytes.len()));
    }
    for &(start, end) in &templates {
        mask.blank(start, end);
    }

    Some(Sfc {
        script: mask.finish()?,
        lang,
        templates,
    })
}

// ── masking ─────────────────────────────────────────────────────────────────

/// A copy of the source that regions can be blanked out of without moving any
/// byte that follows them.
struct Blanked(Vec<u8>);

impl Blanked {
    fn new(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }

    /// Overwrite `[start, end)` with spaces, keeping line breaks so that every
    /// later byte stays on the line it started on.
    fn blank(&mut self, start: usize, end: usize) {
        let end = end.min(self.0.len());
        let start = start.min(end);
        for byte in &mut self.0[start..end] {
            if *byte != b'\n' && *byte != b'\r' {
                *byte = b' ';
            }
        }
    }

    fn finish(self) -> Option<String> {
        String::from_utf8(self.0).ok()
    }
}

// ── block scanning ──────────────────────────────────────────────────────────

/// A `<script>` or `<style>` element.
struct Block {
    start: usize,
    /// Byte range of the open tag's attributes.
    attrs: (usize, usize),
    /// Byte range of the element's content.
    body: (usize, usize),
    end: usize,
}

/// The next `<script>` or `<style>`, whichever comes first, and whether it was
/// a script.
fn next_block(bytes: &[u8], from: usize) -> Option<(Block, bool)> {
    let script = find_element(bytes, from, "script");
    let style = find_element(bytes, from, "style");
    match (script, style) {
        (Some(s), Some(t)) if t.start < s.start => Some((t, false)),
        (Some(s), _) => Some((s, true)),
        (None, Some(t)) => Some((t, false)),
        (None, None) => None,
    }
}

/// The next element with this tag name, at or after `from`.
fn find_element(bytes: &[u8], from: usize, name: &str) -> Option<Block> {
    let open_needle = format!("<{name}");
    let close_needle = format!("</{name}");
    let mut at = from;
    loop {
        let start = find_ignoring_case(bytes, at, open_needle.as_bytes())?;
        let after_name = start + open_needle.len();
        // `<scriptlet>` is not `<script>`.
        match bytes.get(after_name) {
            Some(byte) if byte.is_ascii_whitespace() || *byte == b'>' || *byte == b'/' => {}
            _ => {
                at = start + 1;
                continue;
            }
        }
        let open_end = find_tag_end(bytes, after_name)?;
        let body_start = open_end + 1;
        // A self-closing `<script />` has no body and no closing tag.
        if bytes.get(open_end.wrapping_sub(1)) == Some(&b'/') {
            return Some(Block {
                start,
                attrs: (after_name, open_end.saturating_sub(1)),
                body: (body_start, body_start),
                end: body_start,
            });
        }
        let (body_end, end) = match find_ignoring_case(bytes, body_start, close_needle.as_bytes()) {
            Some(close) => (
                close,
                find_tag_end(bytes, close).map_or(bytes.len(), |gt| gt + 1),
            ),
            // An unclosed block runs to the end of the file.
            None => (bytes.len(), bytes.len()),
        };
        return Some(Block {
            start,
            attrs: (after_name, open_end),
            body: (body_start, body_end),
            end,
        });
    }
}

/// The `>` that closes a tag opened before `from`, ignoring one inside a
/// quoted attribute value.
fn find_tag_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut quote = 0u8;
    for (offset, &byte) in bytes.iter().enumerate().skip(from) {
        if quote != 0 {
            if byte == quote {
                quote = 0;
            }
        } else if byte == b'"' || byte == b'\'' {
            quote = byte;
        } else if byte == b'>' {
            return Some(offset);
        }
    }
    None
}

/// The first `---` fence's closing position, when the file opens with one.
fn astro_frontmatter(bytes: &[u8]) -> Option<usize> {
    if !bytes.starts_with(b"---") {
        return None;
    }
    // The fence closes a line of its own, so a `---` inside an expression or a
    // string in the frontmatter does not end it.
    let mut at = 3;
    while let Some(newline) = find(bytes, at, b"\n") {
        let line = newline + 1;
        if bytes[line..].starts_with(b"---") {
            return Some(line);
        }
        at = line;
    }
    None
}

/// A case-insensitive substring search, ASCII only — every tag name it is
/// given is ASCII.
fn find_ignoring_case(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    let last = haystack.len().checked_sub(needle.len())?;
    (from..=last).find(|&start| {
        haystack[start..start + needle.len()]
            .iter()
            .zip(needle)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

fn find(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    let last = haystack.len().checked_sub(needle.len())?;
    (from..=last).find(|&start| &haystack[start..start + needle.len()] == needle)
}

/// The value of one attribute in an open tag's attribute text.
fn attribute_value(attrs: &[u8], name: &str) -> Option<String> {
    let text = std::str::from_utf8(attrs).ok()?;
    let mut rest = text;
    while let Some(at) = rest.find(name) {
        let before_is_boundary = rest[..at]
            .chars()
            .next_back()
            .is_none_or(|c| c.is_whitespace());
        let after = rest[at + name.len()..].trim_start();
        if before_is_boundary && let Some(value) = after.strip_prefix('=') {
            let value = value.trim_start();
            let quote = value.chars().next()?;
            if quote == '"' || quote == '\'' {
                return value[1..].split(quote).next().map(str::to_string);
            }
        }
        rest = &rest[at + name.len()..];
    }
    None
}

// ── markup scanning ─────────────────────────────────────────────────────────

/// Names in the markup that are the language, not the program.
const KEYWORDS: &[&str] = &[
    "true",
    "false",
    "null",
    "undefined",
    "this",
    "if",
    "else",
    "in",
    "of",
    "for",
    "return",
    "new",
    "typeof",
    "instanceof",
    "void",
    "delete",
    "await",
    "async",
    "function",
    "const",
    "let",
    "var",
    "class",
    "extends",
    "super",
    "try",
    "catch",
    "finally",
    "throw",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "do",
    "while",
    "yield",
    "import",
    "export",
    "from",
    "as",
    "then",
    "each",
    "key",
    "snippet",
    "render",
    "html",
    "debug",
];

/// Attributes that belong to HTML rather than to any component.
const HTML_ATTRIBUTES: &[&str] = &[
    "class",
    "id",
    "style",
    "href",
    "src",
    "alt",
    "rel",
    "target",
    "role",
    "tabindex",
    "xmlns",
    "charset",
    "content",
    "method",
    "action",
    "for",
    "colspan",
    "rowspan",
    "viewbox",
    "fill",
    "stroke",
    "srcset",
    "sizes",
    "loading",
    "decoding",
    "autocomplete",
    "enctype",
    "novalidate",
    "download",
    "hreflang",
    "media",
    "integrity",
    "crossorigin",
    "referrerpolicy",
];

/// Literal `import('…')` specifiers anywhere in a stretch of markup.
///
/// Deliberately blunt: the markup is not parsed as JavaScript, so this looks
/// for the call and takes the quoted argument. A computed specifier has no
/// literal to take and is skipped, which matches what the script analyzer
/// does with the same shape.
fn scan_dynamic_imports(text: &str, base: usize, out: &mut Vec<(String, usize)>) {
    let bytes = text.as_bytes();
    let mut at = 0usize;
    while let Some(found) = text[at..].find("import") {
        let start = at + found;
        at = start + 6;
        // `import` has to be the whole word, and the call its own.
        if start > 0 && is_identifier_byte(bytes[start - 1]) {
            continue;
        }
        let rest = text[at..].trim_start();
        let Some(rest) = rest.strip_prefix('(') else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(quote) = rest.chars().next() else {
            continue;
        };
        if quote != '\'' && quote != '"' {
            continue;
        }
        let Some(specifier) = rest[1..].split(quote).next() else {
            continue;
        };
        if specifier.starts_with('.') && !specifier.is_empty() {
            out.push((specifier.to_string(), base + start));
        }
    }
}

/// Walk markup, reporting every name it reads.
fn scan_markup(text: &str, base: usize, emit: &mut impl FnMut(String, ReferenceKind, usize)) {
    let bytes = text.as_bytes();
    let mut at = 0usize;
    while at < bytes.len() {
        match bytes[at] {
            b'<' if bytes[at..].starts_with(b"<!--") => {
                at = find(bytes, at + 4, b"-->").map_or(bytes.len(), |end| end + 3);
            }
            b'<' if matches!(bytes.get(at + 1), Some(byte) if byte.is_ascii_alphabetic()) => {
                at = scan_tag(bytes, at + 1, base, emit);
            }
            // A closing tag, a doctype, or a stray `<` in prose.
            b'<' => at = find_tag_end(bytes, at + 1).map_or(bytes.len(), |end| end + 1),
            // `{{ vue }}` and `{ svelte }` alike: the brace matcher counts
            // depth, so the doubled form is the single form one level in.
            b'{' => {
                let close = matching_brace(bytes, at);
                scan_expression(bytes, at + 1, close, base, emit);
                at = close.saturating_add(1).min(bytes.len());
            }
            _ => at += 1,
        }
    }
}

/// One element's name and attributes; returns the offset just past the tag.
fn scan_tag(
    bytes: &[u8],
    from: usize,
    base: usize,
    emit: &mut impl FnMut(String, ReferenceKind, usize),
) -> usize {
    let mut at = skip_while(bytes, from, is_tag_name_byte);
    if let Ok(name) = std::str::from_utf8(&bytes[from..at]) {
        emit_element_name(name, base + from, emit);
    }
    loop {
        at = skip_while(bytes, at, |b| b.is_ascii_whitespace());
        match bytes.get(at) {
            None => return at,
            Some(b'>') => return at + 1,
            Some(b'/') => at += 1,
            // `<Foo {...rest} />`, `<Foo {value} />`.
            Some(b'{') => {
                let close = matching_brace(bytes, at);
                scan_expression(bytes, at + 1, close, base, emit);
                at = close.saturating_add(1).min(bytes.len());
            }
            Some(_) => {
                let name_start = at;
                at = skip_while(bytes, at, |b| {
                    !b.is_ascii_whitespace() && !matches!(b, b'=' | b'>' | b'/')
                });
                if at == name_start {
                    // Nothing consumed — a stray `=` or `/`. Step over it
                    // rather than spin.
                    at += 1;
                    continue;
                }
                if let Ok(raw) = std::str::from_utf8(&bytes[name_start..at])
                    && let Some((name, kind)) = attribute_reference(raw)
                {
                    emit(name, kind, base + name_start);
                }
                at = scan_attribute_value(bytes, at, base, emit);
            }
        }
    }
}

/// The `="…"` after an attribute name, if there is one.
fn scan_attribute_value(
    bytes: &[u8],
    from: usize,
    base: usize,
    emit: &mut impl FnMut(String, ReferenceKind, usize),
) -> usize {
    let at = skip_while(bytes, from, |b| b.is_ascii_whitespace());
    if bytes.get(at) != Some(&b'=') {
        return at;
    }
    let at = skip_while(bytes, at + 1, |b| b.is_ascii_whitespace());
    match bytes.get(at) {
        Some(&quote @ (b'"' | b'\'')) => {
            let start = at + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end] != quote {
                end += 1;
            }
            scan_expression(bytes, start, end, base, emit);
            end.saturating_add(1).min(bytes.len())
        }
        Some(b'{') => {
            let close = matching_brace(bytes, at);
            scan_expression(bytes, at + 1, close, base, emit);
            close.saturating_add(1).min(bytes.len())
        }
        _ => skip_while(bytes, at, |b| !b.is_ascii_whitespace() && b != b'>'),
    }
}

/// Every identifier in a stretch of expression, minus string contents.
fn scan_expression(
    bytes: &[u8],
    from: usize,
    to: usize,
    base: usize,
    emit: &mut impl FnMut(String, ReferenceKind, usize),
) {
    let to = to.min(bytes.len());
    let mut at = from.min(to);
    while at < to {
        let byte = bytes[at];
        // A string inside a template expression names nothing a declaration
        // has to answer for; the JS analyzer collects string evidence itself.
        if byte == b'"' || byte == b'\'' || byte == b'`' {
            at += 1;
            while at < to && bytes[at] != byte {
                at += if bytes[at] == b'\\' { 2 } else { 1 };
            }
            at += 1;
            continue;
        }
        if !is_identifier_start(byte) {
            at += 1;
            continue;
        }
        let start = at;
        while at < to && is_identifier_byte(bytes[at]) {
            at += 1;
        }
        let Ok(name) = std::str::from_utf8(&bytes[start..at]) else {
            continue;
        };
        if KEYWORDS.contains(&name) {
            continue;
        }
        // `a.b` reads `b` off whatever `a` is; `...rest` does not.
        let after_dot = start > 0
            && bytes[start - 1] == b'.'
            && bytes.get(start.wrapping_sub(2)) != Some(&b'.');
        let kind = if after_dot {
            ReferenceKind::Member
        } else {
            ReferenceKind::Binding
        };
        emit(name.to_string(), kind, base + start);
    }
}

/// The `}` matching the `{` at `open`, or the end of the buffer.
fn matching_brace(bytes: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    for (at, byte) in outside_strings(bytes, open) {
        if byte == b'{' {
            depth += 1;
        } else if byte == b'}' {
            depth -= 1;
            if depth == 0 {
                return at;
            }
        }
    }
    bytes.len()
}

/// `<Foo>`, `<Foo.Bar>`, `<my-widget>`.
fn emit_element_name(name: &str, at: usize, emit: &mut impl FnMut(String, ReferenceKind, usize)) {
    for part in name.split('.') {
        if part.is_empty() {
            continue;
        }
        emit(part.to_string(), ReferenceKind::Binding, at);
        // Vue and Astro accept `<my-widget>` for a component declared
        // `MyWidget`, and Vue's own style guide prefers it in templates.
        if let Some(pascal) = pascal_case(part) {
            emit(pascal, ReferenceKind::Binding, at);
        }
    }
}

/// `my-widget` → `MyWidget`. `None` when the name is not kebab-case.
fn pascal_case(name: &str) -> Option<String> {
    if !name.contains('-') {
        return None;
    }
    let mut out = String::with_capacity(name.len());
    for word in name.split('-').filter(|word| !word.is_empty()) {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    (!out.is_empty()).then_some(out)
}

/// What an attribute name says about a declaration, if anything.
///
/// A prop is a member read — it is how a parent reaches a child's `export let`
/// or `defineProps` entry. A Svelte action or transition names a function in
/// scope, which is a binding. Events, slots and plain HTML are neither.
fn attribute_reference(raw: &str) -> Option<(String, ReferenceKind)> {
    if raw.starts_with('@') || raw.starts_with('#') {
        return None;
    }
    if let Some(rest) = raw.strip_prefix("v-") {
        // Of Vue's directives only `v-bind:x` names a prop; `v-if`, `v-for`
        // and the rest keep everything they read in the value.
        let (directive, argument) = rest.split_once(':')?;
        if directive != "bind" {
            return None;
        }
        return identifier(argument, ReferenceKind::Member);
    }
    let (prefix, name) = match raw.split_once(':') {
        Some(split) => split,
        None => ("", raw),
    };
    match prefix {
        // `:prop`, `bind:prop`, or a plain attribute that may be a prop.
        "" | "bind" => {
            if HTML_ATTRIBUTES.contains(&name.to_ascii_lowercase().as_str())
                || name.starts_with("data-")
                || name.starts_with("aria-")
            {
                return None;
            }
            identifier(name, ReferenceKind::Member)
        }
        // Svelte reaches a function through these.
        "use" | "transition" | "in" | "out" | "animate" => identifier(name, ReferenceKind::Binding),
        // `on:`, `class:`, `style:`, `client:`, `set:`, `slot:`, `xlink:`.
        _ => None,
    }
}

/// The head of a modified attribute name (`prop.sync`, `fade|local`), when it
/// could be an identifier at all.
fn identifier(raw: &str, kind: ReferenceKind) -> Option<(String, ReferenceKind)> {
    let name = raw.split(['.', '|']).next().unwrap_or(raw);
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first.is_alphabetic() || first == '_' || first == '$') {
        return None;
    }
    if !chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '-') {
        return None;
    }
    if KEYWORDS.contains(&name) {
        return None;
    }
    Some((name.to_string(), kind))
}

fn is_tag_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'$') || is_non_ascii(byte)
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$') || is_non_ascii(byte)
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$') || is_non_ascii(byte)
}

/// Part of a multi-byte character.
///
/// JavaScript identifiers are Unicode, and a codebase that names things in
/// Ukrainian or Japanese must not have its markup read as if the names were
/// not there. Every byte of a multi-byte character is >= 0x80 and no ASCII
/// delimiter is, so taking a run of them as one identifier both keeps the
/// name whole and leaves the run valid UTF-8. Punctuation above ASCII — an
/// em dash in prose — comes through as a name no declaration answers to,
/// which costs nothing.
fn is_non_ascii(byte: u8) -> bool {
    byte >= 0x80
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(source: &str, format: &str) -> Vec<(String, ReferenceKind)> {
        let sfc = split(source, format).expect("a component");
        let lines = LineIndex::new(source.as_bytes());
        sfc.template_references(source, ModuleId(0), &lines)
            .into_iter()
            .map(|reference| (reference.name, reference.kind))
            .collect()
    }

    fn reads(source: &str, format: &str, name: &str) -> bool {
        refs(source, format)
            .iter()
            .any(|(found, kind)| found == name && *kind == ReferenceKind::Binding)
    }

    const VUE: &str = "<template>\n  <my-widget :total=\"sum\" />\n</template>\n\n<script setup lang=\"ts\">\nimport MyWidget from './MyWidget.vue';\nconst sum = 1;\n</script>\n\n<style>.a { color: red }</style>\n";

    #[test]
    fn the_masked_script_keeps_the_length_and_the_lines_of_the_file() {
        for (source, format) in [
            (VUE, "vue"),
            ("<script>let a = 1;</script>\n<p>{a}</p>\n", "svelte"),
            ("---\nconst a = 1;\n---\n<p>{a}</p>\n", "astro"),
        ] {
            let sfc = split(source, format).expect("a component");
            assert_eq!(sfc.script.len(), source.len(), "{format}");
            assert_eq!(
                sfc.script.lines().count(),
                source.lines().count(),
                "{format}"
            );
        }
    }

    #[test]
    fn only_the_script_survives_the_mask() {
        let sfc = split(VUE, "vue").expect("a component");
        assert!(sfc.script.contains("import MyWidget"));
        assert!(sfc.script.contains("const sum = 1;"));
        // Markup and styles would not parse as JavaScript.
        assert!(!sfc.script.contains("my-widget"));
        assert!(!sfc.script.contains("color"));
    }

    #[test]
    fn a_component_used_only_by_the_template_is_read() {
        // The whole point: `<my-widget />` is the only use of the import, and
        // Vue lets the template write it in kebab-case.
        assert!(reads(VUE, "vue", "MyWidget"));
        assert!(reads(VUE, "vue", "sum"));
    }

    #[test]
    fn an_interpolation_is_read_in_either_dialect() {
        assert!(reads(
            "<template><p>{{ total }}</p></template><script setup>const total = 1;</script>",
            "vue",
            "total"
        ));
        assert!(reads(
            "<script>let total = 1;</script><p>{total}</p>",
            "svelte",
            "total"
        ));
        assert!(reads(
            "---\nconst total = 1;\n---\n<p>{total}</p>\n",
            "astro",
            "total"
        ));
    }

    #[test]
    fn a_name_in_a_template_string_is_not_a_reference() {
        // The JS analyzer collects string evidence itself; markup repeating it
        // would make every quoted word look like a use.
        let found = refs(
            "<script>let a = 1;</script>\n<p>{ label('helper') }</p>\n",
            "svelte",
        );
        assert!(found.iter().any(|(name, _)| name == "label"));
        assert!(!found.iter().any(|(name, _)| name == "helper"));
    }

    #[test]
    fn a_prop_is_a_member_read_and_an_action_is_a_binding() {
        let found = refs(
            "<script>import { tip } from './a';</script>\n<Child greeting=\"hi\" use:tip />\n",
            "svelte",
        );
        assert!(found.contains(&("greeting".to_string(), ReferenceKind::Member)));
        assert!(found.contains(&("tip".to_string(), ReferenceKind::Binding)));
        assert!(found.contains(&("Child".to_string(), ReferenceKind::Binding)));
    }

    #[test]
    fn html_attributes_and_events_name_nothing() {
        let found = refs(
            "<div class=\"card\" data-id=\"1\" @click=\"go\" on:keyup={go} />",
            "vue",
        );
        for noise in ["class", "data-id", "click", "keyup"] {
            assert!(
                !found.iter().any(|(name, _)| name == noise),
                "{noise} should not be a reference"
            );
        }
        // The handler itself is still read.
        assert!(found.iter().any(|(name, _)| name == "go"));
    }

    #[test]
    fn a_unicode_identifier_survives_the_scan() {
        // A codebase that names things in Ukrainian must not read as dead.
        assert!(reads(
            "<template><p>{{ ціна }}</p></template>\n<script setup>const ціна = 1;</script>\n",
            "vue",
            "ціна"
        ));
    }

    #[test]
    fn both_vue_script_blocks_are_kept() {
        let sfc = split(
            "<script>export default { name: 'A' };</script>\n<script setup>const b = 1;</script>\n",
            "vue",
        )
        .expect("a component");
        assert!(sfc.script.contains("export default"));
        assert!(sfc.script.contains("const b = 1;"));
    }

    #[test]
    fn astro_frontmatter_is_the_script_and_a_client_script_is_not() {
        let sfc = split(
            "---\nimport A from './A.astro';\n---\n<A />\n<script>console.log('client');</script>\n",
            "astro",
        )
        .expect("a component");
        assert!(sfc.script.contains("import A from './A.astro';"));
        // Merging a client module into the frontmatter's scope would redeclare
        // names; the block is dropped instead.
        assert!(!sfc.script.contains("console.log"));
        assert!(reads(
            "---\nimport A from './A.astro';\n---\n<A />\n",
            "astro",
            "A"
        ));
    }

    #[test]
    fn a_file_with_no_frontmatter_and_no_script_is_still_a_component() {
        let sfc = split("<h1>plain</h1>\n", "astro").expect("a component");
        assert_eq!(sfc.script.trim(), "");
        let sfc = split("<template><p>plain</p></template>\n", "vue").expect("a component");
        assert_eq!(sfc.script.trim(), "");
    }

    #[test]
    fn a_fence_inside_the_frontmatter_does_not_end_it() {
        let sfc = split(
            "---\nconst rule = '---';\nconst after = 1;\n---\n<p />\n",
            "astro",
        )
        .expect("a component");
        assert!(sfc.script.contains("const after = 1;"));
    }

    #[test]
    fn malformed_markup_terminates() {
        // Unclosed tags, a stray `<`, an unbalanced brace: the scanner must
        // make progress on all of it rather than spin.
        for source in [
            "<template>\n  <div class=\"x\" <span>\n  5 < 6 { unbalanced\n  <img src=\"a\"\n",
            "<script>let a = 1;",
            "<template>{{",
            "---\nconst a = 1;\n",
            "<div ==/>< >{}",
        ] {
            for format in ["vue", "svelte", "astro"] {
                let sfc = split(source, format).expect("a component");
                let lines = LineIndex::new(source.as_bytes());
                let _ = sfc.template_references(source, ModuleId(0), &lines);
                assert_eq!(sfc.script.len(), source.len());
            }
        }
    }

    #[test]
    fn a_script_block_decides_the_source_type() {
        let ts = split("<script lang=\"ts\">let a: number = 1;</script>", "vue").unwrap();
        assert!(ts.source_type().is_typescript());
        let tsx = split("<script lang=\"tsx\">let a = <b />;</script>", "vue").unwrap();
        assert!(tsx.source_type().is_jsx());
    }

    #[test]
    fn only_component_formats_are_split() {
        assert!(split("const a = 1;", "typescript").is_none());
        assert!(split("const a = 1;", "javascript").is_none());
        for format in SFC_FORMATS {
            assert!(split("<p />", format).is_some(), "{format}");
        }
    }

    #[test]
    fn an_import_written_in_the_markup_is_still_an_import() {
        // `{#await import('./Heavy.svelte')}` is blanked out of the script
        // buffer, so nothing else in the crate can see it.
        let source = "<script>let a = 1;</script>\n{#await import('./Heavy.svelte') then { default: H }}<H />{/await}\n";
        let sfc = split(source, "svelte").expect("a component");
        let found: Vec<_> = sfc
            .template_imports(source)
            .into_iter()
            .map(|(specifier, _)| specifier)
            .collect();
        assert_eq!(found, vec!["./Heavy.svelte".to_string()]);
    }

    #[test]
    fn a_computed_or_bare_markup_import_names_nothing() {
        for source in [
            "<p>{#await import(name)}</p>",
            "<p>{#await import(`./${x}.svelte`)}</p>",
            "<p>{#await import('svelte')}</p>",
            "<p>reimport('./x.svelte')</p>",
        ] {
            let sfc = split(source, "svelte").expect("a component");
            assert!(sfc.template_imports(source).is_empty(), "{source}");
        }
    }

    #[test]
    fn kebab_case_becomes_the_declared_name() {
        assert_eq!(pascal_case("my-widget").as_deref(), Some("MyWidget"));
        assert_eq!(pascal_case("a-b-c").as_deref(), Some("ABC"));
        assert_eq!(pascal_case("Widget"), None);
    }
}
