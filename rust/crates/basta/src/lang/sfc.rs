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
    /// Bodies of Astro's client `<script>` blocks — modules Astro bundles on
    /// their own, outside the frontmatter's scope.
    client_scripts: Vec<(usize, usize)>,
    /// `src` of Astro client scripts that load a file instead, with the
    /// offset of the tag.
    client_sources: Vec<(String, usize)>,
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
    /// Only expressions are searched: the same words in a comment or in an
    /// attribute's plain text import nothing.
    pub fn template_imports(&self, source: &str) -> Vec<(String, usize)> {
        let mut out = Vec::new();
        self.walk_markup(source, &mut |found, at| {
            if let MarkupUse::Import(specifier) = found {
                out.push((specifier, at));
            }
        });
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
        self.walk_markup(source, &mut |found, at| {
            if let MarkupUse::Read(name, kind) = found {
                out.push(Reference {
                    module,
                    name,
                    kind,
                    // The markup is the component's body: it runs whenever the
                    // component does, which is what module top level means here.
                    from: None,
                    at: lines.location(at),
                });
            }
        });
        out
    }

    /// Each Astro client script as a buffer of the file's length with
    /// everything but that script blanked, so positions stay real.
    ///
    /// Astro bundles these as separate modules. Merging one into the
    /// frontmatter's scope would redeclare names, so it is read on its own;
    /// dropping it would leave every module it imports looking unused.
    pub fn client_scripts(&self, source: &str) -> Vec<String> {
        self.client_scripts
            .iter()
            .filter_map(|&(start, end)| {
                let mut mask = Blanked::new(source.as_bytes());
                mask.blank(0, start);
                mask.blank(end, source.len());
                mask.finish()
            })
            .collect()
    }

    /// Files Astro client scripts load through `src`, with the tag's offset.
    pub fn client_sources(&self) -> &[(String, usize)] {
        &self.client_sources
    }

    fn walk_markup(&self, source: &str, emit: &mut impl FnMut(MarkupUse, usize)) {
        for &(start, end) in &self.templates {
            if let Some(text) = source.get(start..end) {
                scan_markup(text, start, emit);
            }
        }
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
    let mut client_scripts: Vec<(usize, usize)> = Vec::new();
    let mut client_sources: Vec<(String, usize)> = Vec::new();
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
        // names, so it is kept apart and read on its own.
        if is_script
            && format == "astro"
            && let Some(src) = astro_processed_script(&bytes[block.attrs.0..block.attrs.1])
        {
            match src {
                Some(src) => client_sources.push((src, block.start)),
                None => client_scripts.push(block.body),
            }
        }
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
        client_scripts,
        client_sources,
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
/// quoted attribute value or inside a `{…}` expression.
///
/// Svelte and Astro write attribute values as bare expressions, and an event
/// handler is an arrow function: `on:load={() => { loaded = true }}`. The `>`
/// of that `=>` is not the end of the tag. Read as one, it cut a
/// `<script src=…>` in a layout's `<svelte:head>` in half and handed the rest
/// of the attribute to the JavaScript parser as the component's script — the
/// file failed to parse, and everything it imports was reported as unused.
fn find_tag_end(bytes: &[u8], from: usize) -> Option<usize> {
    let (mut quote, mut depth) = (0u8, 0usize);
    for (offset, &byte) in bytes.iter().enumerate().skip(from) {
        if quote != 0 {
            if byte == quote {
                quote = 0;
            }
            continue;
        }
        match byte {
            b'"' | b'\'' | b'`' if depth > 0 || byte != b'`' => quote = byte,
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b'>' if depth == 0 => return Some(offset),
            _ => {}
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

/// Whether Astro bundles a client `<script>` with these attributes, and the
/// file it loads when it has a `src`.
///
/// Astro processes a script only when it carries no attribute but `src`; any
/// other attribute (`is:inline`, `type="module"`, `define:vars`) leaves it as
/// written, served to the browser and resolved there, not from the tree.
fn astro_processed_script(attrs: &[u8]) -> Option<Option<String>> {
    let text = std::str::from_utf8(attrs).ok()?.trim();
    if text.is_empty() {
        return Some(None);
    }
    let src = attribute_value(attrs, "src")?;
    let rest =
        text.replacen(&format!("src=\"{src}\""), "", 1)
            .replacen(&format!("src='{src}'"), "", 1);
    rest.trim().is_empty().then_some(Some(src))
}

// ── markup scanning ─────────────────────────────────────────────────────────

/// What the markup walk found at an offset.
enum MarkupUse {
    /// A name the markup reads.
    Read(String, ReferenceKind),
    /// A literal `import('./x')` written in an expression.
    Import(String),
}

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

/// Walk markup, reporting every name it reads.
fn scan_markup(text: &str, base: usize, emit: &mut impl FnMut(MarkupUse, usize)) {
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
    emit: &mut impl FnMut(MarkupUse, usize),
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
                    emit(MarkupUse::Read(name, kind), base + name_start);
                }
                let name = std::str::from_utf8(&bytes[name_start..at]).unwrap_or("");
                at = scan_attribute_value(bytes, at, base, is_directive(name), emit);
            }
        }
    }
}

/// The `="…"` after an attribute name, if there is one.
///
/// A quoted value is JavaScript only on a directive — `:prop`, `@click`,
/// `#slot`, `v-if`. On any other attribute it is text (`class="card"`,
/// `title="unusedName"`), and only the `{…}` interpolations Svelte and Astro
/// allow inside it are expressions; reading the rest as names would credit a
/// declaration that happens to share a word with a CSS class.
fn scan_attribute_value(
    bytes: &[u8],
    from: usize,
    base: usize,
    directive: bool,
    emit: &mut impl FnMut(MarkupUse, usize),
) -> usize {
    let at = skip_while(bytes, from, |b| b.is_ascii_whitespace());
    if bytes.get(at) != Some(&b'=') {
        return at;
    }
    let at = skip_while(bytes, at + 1, |b| b.is_ascii_whitespace());
    match bytes.get(at) {
        Some(&quote @ (b'"' | b'\'')) => {
            let start = at + 1;
            let end = skip_while(bytes, start, |b| b != quote);
            if directive {
                scan_expression(bytes, start, end, base, emit);
            } else {
                scan_interpolations(bytes, start, end, base, emit);
            }
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

/// Whether an attribute's quoted value is an expression: Vue's directives and
/// their shorthands.
fn is_directive(name: &str) -> bool {
    name.starts_with([':', '@', '#']) || name.starts_with("v-")
}

/// The `{…}` expressions inside a stretch of attribute text.
fn scan_interpolations(
    bytes: &[u8],
    from: usize,
    to: usize,
    base: usize,
    emit: &mut impl FnMut(MarkupUse, usize),
) {
    let mut at = from;
    while let Some(open) = (at..to).find(|&i| bytes[i] == b'{') {
        let close = matching_brace(bytes, open).min(to);
        scan_expression(bytes, open + 1, close, base, emit);
        at = close + 1;
    }
}

/// Every identifier in a stretch of expression, minus string contents.
fn scan_expression(
    bytes: &[u8],
    from: usize,
    to: usize,
    base: usize,
    emit: &mut impl FnMut(MarkupUse, usize),
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
        if name == "import"
            && let Some(specifier) = literal_call_argument(bytes, at, to)
        {
            emit(MarkupUse::Import(specifier), base + start);
            continue;
        }
        if KEYWORDS.contains(&name) || is_property_key(bytes, from, start, at, to) {
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
        emit(MarkupUse::Read(name.to_string(), kind), base + start);
    }
}

/// The relative specifier in `('./x')` right after `at`, if that is what
/// follows.
fn literal_call_argument(bytes: &[u8], at: usize, to: usize) -> Option<String> {
    let open = skip_while(&bytes[..to], at, |b| b.is_ascii_whitespace());
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    let quote_at = skip_while(&bytes[..to], open + 1, |b| b.is_ascii_whitespace());
    let quote = *bytes.get(quote_at).filter(|&&q| q == b'\'' || q == b'"')?;
    let end = skip_while(&bytes[..to], quote_at + 1, |b| b != quote);
    let specifier = std::str::from_utf8(bytes.get(quote_at + 1..end)?).ok()?;
    specifier.starts_with('.').then(|| specifier.to_string())
}

/// Whether the identifier at `[start, end)` is an object key written as
/// `{ key: value }` — a name for a slot, not a read of anything.
///
/// Shorthand `{ key }` is still a read, and so is `cond ? a : b`, where the
/// word before the colon follows a `?` rather than a `{` or a `,`.
fn is_property_key(bytes: &[u8], from: usize, start: usize, end: usize, to: usize) -> bool {
    let next = skip_while(&bytes[..to], end, |b| b.is_ascii_whitespace());
    if bytes.get(next) != Some(&b':') || bytes.get(next + 1) == Some(&b':') {
        return false;
    }
    bytes[from..start]
        .iter()
        .rev()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|&b| b == b'{' || b == b',')
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
fn emit_element_name(name: &str, at: usize, emit: &mut impl FnMut(MarkupUse, usize)) {
    for part in name.split('.') {
        if part.is_empty() {
            continue;
        }
        emit(
            MarkupUse::Read(part.to_string(), ReferenceKind::Binding),
            at,
        );
        // Vue and Astro accept `<my-widget>` for a component declared
        // `MyWidget`, and Vue's own style guide prefers it in templates.
        if let Some(pascal) = pascal_case(part) {
            emit(MarkupUse::Read(pascal, ReferenceKind::Binding), at);
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

    /// The specifiers a Svelte component's markup imports.
    fn markup_imports(source: &str) -> Vec<String> {
        split(source, "svelte")
            .expect("a component")
            .template_imports(source)
            .into_iter()
            .map(|(specifier, _)| specifier)
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
    fn an_arrow_function_in_an_attribute_does_not_end_the_tag() {
        // From a real SvelteKit layout: a third-party `<script src>` in the
        // head, with an event handler written as an expression.
        let source = "<script lang=\"ts\">\n  import Sidebar from './Sidebar.svelte';\n  let loaded = false;\n</script>\n\n\
            <svelte:head>\n  <script\n    defer\n    on:load={() => {\n      loaded = true;\n    }}\n    \
            src=\"https://{host}/js/script.js\"\n  ></script>\n</svelte:head>\n<Sidebar />\n";
        let sfc = split(source, "svelte").expect("a component");
        assert!(sfc.script.contains("import Sidebar"));
        assert!(
            !sfc.script.contains("loaded = true"),
            "the handler is an attribute of the tag, not the component's script: {}",
            sfc.script
        );
        assert!(!sfc.script.contains("src="), "{}", sfc.script);
        assert!(reads(source, "svelte", "Sidebar"));
        // A `>` inside a string inside the expression is no tag end either.
        assert_eq!(find_tag_end(b"a={x ? \"a>b\" : `c>d`} b>", 0), Some(23));
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
    fn an_import_in_a_comment_or_in_plain_attribute_text_is_not_an_import() {
        for source in [
            "<!-- {#await import('./dead.svelte')} -->\n<p>hi</p>\n",
            "<p title=\"import('./dead.svelte')\">hi</p>\n",
            "<p>see import('./dead.svelte') in the docs</p>\n",
        ] {
            assert!(markup_imports(source).is_empty(), "{source}");
        }
        // Inside an expression it is a real import, attribute or not.
        let source = "<Lazy loader={() => import('./live.svelte')} />\n";
        assert_eq!(markup_imports(source), vec!["./live.svelte".to_string()]);
    }

    #[test]
    fn a_plain_attribute_value_is_text_and_a_directive_is_an_expression() {
        let vue = "<template><div class=\"unusedName\" title=\"spare\" :data-x=\"usedName\" @click=\"onClick\" v-if=\"visible\" /></template>\n<script setup>const usedName = 1;</script>\n";
        for name in ["usedName", "onClick", "visible"] {
            assert!(reads(vue, "vue", name), "{name} is read by a directive");
        }
        for name in ["unusedName", "spare"] {
            assert!(!reads(vue, "vue", name), "{name} is only text");
        }
        // Svelte and Astro interpolate inside a quoted value.
        let svelte =
            "<script>let active = true;</script>\n<p class=\"card {active ? 'on' : ''}\">x</p>\n";
        assert!(reads(svelte, "svelte", "active"));
        assert!(!reads(svelte, "svelte", "card"));
    }

    #[test]
    fn an_object_key_in_markup_names_a_slot_and_reads_nothing() {
        let source = "<template><p :class=\"{ label: isOn, other: 1 }\" :style=\"{ color }\" /></template>\n";
        assert!(reads(source, "vue", "isOn"));
        assert!(!reads(source, "vue", "label"));
        assert!(!reads(source, "vue", "other"));
        // Shorthand is a read, and so is the middle of a ternary.
        assert!(reads(source, "vue", "color"));
        let svelte = "<p>{flag ? shown : hidden}</p>\n";
        for name in ["flag", "shown", "hidden"] {
            assert!(reads(svelte, "svelte", name), "{name}");
        }
    }

    #[test]
    fn an_astro_client_script_is_read_as_a_module_of_its_own() {
        let source = "---\nconst title = 'x';\n---\n<h1>{title}</h1>\n<script>import { mount } from './widget.ts';\nmount();</script>\n";
        let sfc = split(source, "astro").expect("a component");
        let scripts = sfc.client_scripts(source);
        assert_eq!(scripts.len(), 1);
        let client = &scripts[0];
        assert_eq!(client.len(), source.len(), "positions stay real");
        assert!(client.contains("import { mount } from './widget.ts';"));
        assert!(
            !client.contains("const title"),
            "the frontmatter is not in it"
        );
        // A `src` loads a file instead.
        let source = "<script src=\"./menu.ts\"></script>\n";
        let sfc = split(source, "astro").expect("a component");
        assert_eq!(sfc.client_sources()[0].0, "./menu.ts");
    }

    #[test]
    fn an_astro_script_with_other_attributes_is_left_to_the_browser() {
        // Astro processes a script only when it has no attribute but `src`;
        // anything else is served as written and resolved by the browser.
        for source in [
            "<script is:inline>import './a.js';</script>\n",
            "<script type=\"module\">import './a.js';</script>\n",
            "<script define:vars={{ x }}>import './a.js';</script>\n",
        ] {
            let sfc = split(source, "astro").expect("a component");
            assert!(sfc.client_scripts(source).is_empty(), "{source}");
            assert!(sfc.client_sources().is_empty(), "{source}");
        }
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
        // names; the block is kept apart instead.
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
        assert_eq!(markup_imports(source), vec!["./Heavy.svelte".to_string()]);
    }

    #[test]
    fn a_computed_or_bare_markup_import_names_nothing() {
        for source in [
            "<p>{#await import(name)}</p>",
            "<p>{#await import(`./${x}.svelte`)}</p>",
            "<p>{#await import('svelte')}</p>",
            "<p>reimport('./x.svelte')</p>",
        ] {
            assert!(markup_imports(source).is_empty(), "{source}");
        }
    }

    #[test]
    fn kebab_case_becomes_the_declared_name() {
        assert_eq!(pascal_case("my-widget").as_deref(), Some("MyWidget"));
        assert_eq!(pascal_case("a-b-c").as_deref(), Some("ABC"));
        assert_eq!(pascal_case("Widget"), None);
    }
}
