//! Clojure: every top-level form except `ns` is a unit.
//!
//! A small reader turns the source into forms; it evaluates nothing. Reader
//! conditionals keep their `:clj` branch (or `:default`), `#_` drops the next
//! form, metadata drops out, and quote, syntax-quote, unquote, deref and var
//! quote become lists headed by their names, as `quote` and `deref` are in
//! Clojure itself. A form the reader cannot read ends the file: the forms
//! before it still count.
//!
//! Normalized, a symbol is `:symbol` unless it heads a list, where it stays
//! as `[:symbol "map"]`; a keyword is `:keyword`, every other literal
//! `:literal`, and a map holds `[key value]` pairs.

use crate::prints::{Prints, Value, keyword};

/// A form read from Clojure source.
#[derive(Debug, Clone, PartialEq)]
enum Form {
    Symbol(String),
    /// A keyword, by its name without the colons.
    Keyword(String),
    Literal,
    Coll(Kind, Vec<Form>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    List,
    Vector,
    Set,
    Map,
}

/// A top-level unit: its name, its byte span and its fingerprints.
#[derive(Debug, Clone)]
pub(crate) struct ClojureForm {
    pub name: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub nodes: u32,
    pub fingerprints: Vec<u64>,
}

/// The units of `source` outside the sorted, disjoint byte ranges `ignored`.
pub(crate) fn forms(source: &str, ignored: &[[usize; 2]]) -> Vec<ClojureForm> {
    let mut reader = Reader::new(source);
    let mut out = Vec::new();
    loop {
        reader.skip_whitespace();
        if reader.at_end() {
            break;
        }
        let start = reader.at;
        let Ok(form) = reader.read() else {
            break;
        };
        let Some(form) = form else {
            continue;
        };
        let end = reader.at;
        let Form::Coll(Kind::List, items) = &form else {
            continue;
        };
        let Some(head) = items.first() else {
            continue;
        };
        if matches!(head, Form::Symbol(name) if name == "ns") {
            continue;
        }
        if crate::syntax::inside(ignored, start, end) {
            continue;
        }
        let name = match items.get(1) {
            Some(Form::Symbol(name)) => name.clone(),
            _ => match head {
                Form::Symbol(name) => name.clone(),
                _ => "<form>".to_string(),
            },
        };
        let mut prints = Prints::default();
        let root = normalize(&form, &mut prints);
        let Value::List { nodes, .. } = root else {
            continue;
        };
        out.push(ClojureForm {
            name,
            start_byte: start,
            end_byte: end,
            nodes,
            fingerprints: prints.finish(root),
        });
    }
    out
}

/// The normalized tree of `form`, children before parents with a stack of
/// its own: deeply nested data must not overflow the thread's stack.
fn normalize(form: &Form, prints: &mut Prints) -> Value {
    enum Step<'f> {
        Enter(&'f Form, bool),
        Leave(&'f Form),
    }
    let mut values: Vec<Value> = Vec::new();
    let mut stack = vec![Step::Enter(form, false)];
    while let Some(step) = stack.pop() {
        match step {
            Step::Enter(form, head) => match form {
                Form::Symbol(name) => values.push(match head {
                    true => prints.symbol(name),
                    false => Value::Atom(keyword("symbol")),
                }),
                Form::Keyword(..) => values.push(Value::Atom(keyword("keyword"))),
                Form::Literal => values.push(Value::Atom(keyword("literal"))),
                Form::Coll(kind, items) => {
                    stack.push(Step::Leave(form));
                    for (i, item) in items.iter().enumerate().rev() {
                        stack.push(Step::Enter(item, *kind == Kind::List && i == 0));
                    }
                }
            },
            Step::Leave(form) => {
                let Form::Coll(kind, items) = form else {
                    unreachable!("only collections are left");
                };
                let children = values.split_off(values.len() - items.len());
                let value = match kind {
                    Kind::List if children.is_empty() => prints.list(&[
                        Value::Atom(keyword("list")),
                        Value::Atom(keyword("literal")),
                    ]),
                    Kind::Map => {
                        let mut parts = vec![Value::Atom(keyword("map"))];
                        for pair in children.chunks(2) {
                            parts.push(prints.list(pair));
                        }
                        prints.list(&parts)
                    }
                    _ => {
                        let name = match kind {
                            Kind::List => "list",
                            Kind::Vector => "vector",
                            _ => "set",
                        };
                        let mut parts = vec![Value::Atom(keyword(name))];
                        parts.extend(children);
                        prints.list(&parts)
                    }
                };
                values.push(value);
            }
        }
    }
    values.pop().unwrap_or(Value::None)
}

/// The source cannot be read on from here.
#[derive(Debug)]
struct Unreadable;

/// What a form being read is inside of.
enum Open {
    /// A collection, with the byte that closes it.
    Coll(Kind, Vec<Form>, u8),
    /// `#(…)`: a list headed by `fn*`.
    Function(Vec<Form>),
    /// Quote and the like: a list of the name and the next form.
    Wrap(&'static str),
    /// `#_`: the next form drops out.
    Discard,
    /// `^`: the next form is metadata and drops out, the one after it stays.
    Meta,
    /// A tagged literal: the next form is its value.
    Tagged,
    /// A reader conditional, splicing or not: the next form is its body.
    Conditional(bool),
}

/// What reading a form gave: a form, nothing (a discarded form, a
/// conditional without a `:clj` branch), or forms to splice into the
/// collection around.
enum Read {
    Form(Form),
    Nothing,
    Splice(Vec<Form>),
}

struct Reader<'s> {
    text: &'s [u8],
    at: usize,
}

impl<'s> Reader<'s> {
    fn new(source: &'s str) -> Self {
        let mut text = source.as_bytes();
        if let Some(rest) = text.strip_prefix("\u{feff}".as_bytes()) {
            text = rest;
        }
        let mut reader = Self { text, at: 0 };
        if text.starts_with(b"#!") {
            reader.skip_line();
        }
        reader
    }

    fn at_end(&self) -> bool {
        self.at >= self.text.len()
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.at += 1;
        Some(byte)
    }

    fn skip_line(&mut self) {
        while let Some(byte) = self.peek() {
            if byte == b'\n' {
                break;
            }
            self.bump();
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(byte) = self.peek() {
            if is_whitespace(byte) {
                self.bump();
            } else if byte == b';' {
                self.skip_line();
            } else {
                break;
            }
        }
    }

    /// One top-level form; `None` when it reads as nothing.
    fn read(&mut self) -> Result<Option<Form>, Unreadable> {
        let mut open: Vec<Open> = Vec::new();
        loop {
            let mut read = self.next(&mut open)?;
            // Hand what was read to what is open, innermost first.
            loop {
                let Some(top) = open.last_mut() else {
                    return match read {
                        Read::Form(form) => Ok(Some(form)),
                        Read::Nothing | Read::Splice(_) => Ok(None),
                    };
                };
                match top {
                    Open::Coll(_, items, _) | Open::Function(items) => {
                        match read {
                            Read::Form(form) => items.push(form),
                            Read::Splice(forms) => items.extend(forms),
                            Read::Nothing => {}
                        }
                        break;
                    }
                    Open::Wrap(name) => {
                        let name = *name;
                        open.pop();
                        let mut items = vec![Form::Symbol(name.to_string())];
                        if let Read::Form(form) = read {
                            items.push(form);
                        }
                        read = Read::Form(Form::Coll(Kind::List, items));
                    }
                    Open::Discard => {
                        open.pop();
                        read = Read::Nothing;
                    }
                    Open::Meta => {
                        // The metadata is read; the form it marks comes next.
                        open.pop();
                        break;
                    }
                    Open::Tagged => {
                        open.pop();
                        read = Read::Form(Form::Literal);
                    }
                    Open::Conditional(splicing) => {
                        let splicing = *splicing;
                        open.pop();
                        read = conditional(read, splicing)?;
                    }
                }
            }
        }
    }

    /// The next form at this level: an atom, or a collection that closes
    /// here. Anything that opens is pushed on `open` and reads as nothing.
    fn next(&mut self, open: &mut Vec<Open>) -> Result<Read, Unreadable> {
        loop {
            self.skip_whitespace();
            let byte = self.peek().ok_or(Unreadable)?;
            let close = match open.last() {
                Some(Open::Coll(_, _, close)) => Some(*close),
                Some(Open::Function(..)) => Some(b')'),
                _ => None,
            };
            if close == Some(byte) {
                self.bump();
                return Ok(Read::Form(close_collection(open.pop())?));
            }
            match byte {
                b'(' | b'[' | b'{' => {
                    self.bump();
                    let (kind, close) = match byte {
                        b'(' => (Kind::List, b')'),
                        b'[' => (Kind::Vector, b']'),
                        _ => (Kind::Map, b'}'),
                    };
                    open.push(Open::Coll(kind, Vec::new(), close));
                }
                b')' | b']' | b'}' => return Err(Unreadable),
                b'"' => {
                    self.string()?;
                    return Ok(Read::Form(Form::Literal));
                }
                b'\\' => {
                    self.bump();
                    self.bump().ok_or(Unreadable)?;
                    self.token();
                    return Ok(Read::Form(Form::Literal));
                }
                b':' => {
                    self.bump();
                    if self.peek() == Some(b':') {
                        self.bump();
                    }
                    let name = self.token().to_string();
                    return Ok(Read::Form(Form::Keyword(name)));
                }
                b'\'' | b'@' | b'`' => {
                    self.bump();
                    let name = match byte {
                        b'\'' => "quote",
                        b'@' => "deref",
                        _ => "syntax-quote",
                    };
                    open.push(Open::Wrap(name));
                }
                b'~' => {
                    self.bump();
                    let name = match self.peek() {
                        Some(b'@') => {
                            self.bump();
                            "unquote-splicing"
                        }
                        _ => "unquote",
                    };
                    open.push(Open::Wrap(name));
                }
                b'^' => {
                    self.bump();
                    open.push(Open::Meta);
                }
                b'#' => {
                    self.bump();
                    if let Some(read) = self.dispatch(open)? {
                        return Ok(read);
                    }
                }
                _ => {
                    let token = self.token();
                    if token.is_empty() {
                        return Err(Unreadable);
                    }
                    return Ok(Read::Form(match is_number(token) {
                        true => Form::Literal,
                        false => Form::Symbol(token.to_string()),
                    }));
                }
            }
        }
    }

    /// What follows a `#`: a form when it reads one at once, `None` when it
    /// opened something.
    fn dispatch(&mut self, open: &mut Vec<Open>) -> Result<Option<Read>, Unreadable> {
        let byte = self.peek().ok_or(Unreadable)?;
        match byte {
            b'_' => {
                self.bump();
                open.push(Open::Discard);
            }
            b'{' => {
                self.bump();
                open.push(Open::Coll(Kind::Set, Vec::new(), b'}'));
            }
            b'(' => {
                self.bump();
                open.push(Open::Function(Vec::new()));
            }
            b'"' => {
                self.string()?;
                return Ok(Some(Read::Form(Form::Literal)));
            }
            b'\'' => {
                self.bump();
                open.push(Open::Wrap("var"));
            }
            b'?' => {
                self.bump();
                let splicing = self.peek() == Some(b'@');
                if splicing {
                    self.bump();
                }
                open.push(Open::Conditional(splicing));
            }
            b':' => {
                // A namespaced map: `#:ns{…}` or `#::{…}`, read as a map.
                self.bump();
                if self.peek() == Some(b':') {
                    self.bump();
                }
                self.token();
            }
            b'^' => {
                self.bump();
                open.push(Open::Meta);
            }
            b'!' => self.skip_line(),
            b'#' => {
                // `##Inf`, `##NaN`.
                self.bump();
                self.token();
                return Ok(Some(Read::Form(Form::Literal)));
            }
            b'=' => {
                self.bump();
                open.push(Open::Tagged);
            }
            _ => {
                if self.token().is_empty() {
                    return Err(Unreadable);
                }
                open.push(Open::Tagged);
            }
        }
        Ok(None)
    }

    fn string(&mut self) -> Result<(), Unreadable> {
        self.bump();
        while let Some(byte) = self.bump() {
            match byte {
                b'\\' => {
                    self.bump();
                }
                b'"' => return Ok(()),
                _ => {}
            }
        }
        Err(Unreadable)
    }

    /// A symbol or number: everything up to whitespace or a byte that
    /// starts another form. `#`, `'` and `:` go on a symbol, as in `x#`.
    fn token(&mut self) -> &'s str {
        let start = self.at;
        while let Some(byte) = self.peek() {
            if is_whitespace(byte) || is_terminator(byte) {
                break;
            }
            self.bump();
        }
        std::str::from_utf8(&self.text[start..self.at]).unwrap_or_default()
    }
}

fn close_collection(open: Option<Open>) -> Result<Form, Unreadable> {
    match open {
        Some(Open::Coll(Kind::Map, items, _)) => {
            if items.len() % 2 == 1 {
                return Err(Unreadable);
            }
            Ok(Form::Coll(Kind::Map, items))
        }
        Some(Open::Coll(kind, items, _)) => Ok(Form::Coll(kind, items)),
        Some(Open::Function(items)) => {
            let mut all = vec![Form::Symbol("fn*".to_string())];
            all.extend(items);
            Ok(Form::Coll(Kind::List, all))
        }
        _ => Err(Unreadable),
    }
}

/// The branch of a reader conditional that Clojure on the JVM reads: the
/// `:clj` one, or `:default`; spliced into the collection around for `#?@`.
fn conditional(body: Read, splicing: bool) -> Result<Read, Unreadable> {
    let Read::Form(Form::Coll(Kind::List, items)) = body else {
        return Err(Unreadable);
    };
    let chosen = items
        .chunks(2)
        .find(|pair| matches!(pair, [Form::Keyword(name), _] if name == "clj" || name == "default"))
        .and_then(|pair| pair.get(1).cloned());
    let Some(chosen) = chosen else {
        return Ok(Read::Nothing);
    };
    if !splicing {
        return Ok(Read::Form(chosen));
    }
    match chosen {
        Form::Coll(Kind::List | Kind::Vector, items) => Ok(Read::Splice(items)),
        _ => Err(Unreadable),
    }
}

fn is_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b',')
}

fn is_terminator(byte: u8) -> bool {
    matches!(
        byte,
        b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'"' | b';' | b'@' | b'^' | b'`' | b'~' | b'\\'
    )
}

/// Whether a token is a number: an integer, a ratio, a decimal or a hex
/// number, with an optional sign and `M` or `N` suffix.
fn is_number(token: &str) -> bool {
    let body = token
        .strip_prefix(['+', '-'])
        .unwrap_or(token)
        .trim_end_matches(['M', 'N']);
    if body.is_empty() || !(body.as_bytes()[0].is_ascii_digit() || body.starts_with('.')) {
        return false;
    }
    if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        return !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit());
    }
    if let Some((numerator, denominator)) = body.split_once('/') {
        return !numerator.is_empty()
            && !denominator.is_empty()
            && numerator.bytes().all(|b| b.is_ascii_digit())
            && denominator.bytes().all(|b| b.is_ascii_digit());
    }
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(at) => (&body[..at], Some(&body[at + 1..])),
        None => (body, None),
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok = match mantissa.split_once('.') {
        Some((whole, fraction)) => (whole.is_empty() || digits(whole)) && digits(fraction),
        None => digits(mantissa),
    };
    let exponent_ok = exponent.is_none_or(|e| digits(e.strip_prefix(['+', '-']).unwrap_or(e)));
    mantissa_ok && exponent_ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::join::jaccard;

    #[test]
    fn locals_and_mapped_functions_normalize_away_but_the_heads_stay() {
        let source = "(ns demo.core)\n\n(defn alpha [xs]\n  (let [ys (filter odd? xs)]\n    (map inc ys)))\n\n(defn beta [items]\n  (let [kept (filter even? items)]\n    (map dec kept)))\n\n(defn gamma [items]\n  (let [kept (remove even? items)]\n    (map dec kept)))\n";
        let units = forms(source, &[]);
        let names: Vec<&str> = units.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["alpha", "beta", "gamma"]);
        assert_eq!(jaccard(&units[0].fingerprints, &units[1].fingerprints), 1.0);
        assert!(jaccard(&units[1].fingerprints, &units[2].fingerprints) < 1.0);
        assert_eq!(&source[units[0].start_byte..][..12], "(defn alpha ");
    }

    #[test]
    fn reader_syntax_reads_as_clojure_does() {
        let source = "(defmacro m [x] `(let [y# ~x] (+ y# 1)))\n#_(drop me)\n(def ^:private table #?(:cljs [] :clj {:a 1 :b #{2}}))\n(f #(inc %) #'g @h #inst \"2020\" \\a ##Inf 1/2 -3.5e2)\n";
        let units = forms(source, &[]);
        let names: Vec<&str> = units.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["m", "table", "f"]);
    }

    #[test]
    fn an_unreadable_form_ends_the_file() {
        let units = forms("(defn a [] 1)\n(defn b [] {:odd})\n(defn c [] 3)\n", &[]);
        assert_eq!(units.len(), 1);
    }

    #[test]
    fn numbers_are_literals_and_names_are_not() {
        for number in ["1", "-2", "+3N", "4.5", ".5", "6e7", "1/2", "0xFF", "8M"] {
            assert!(is_number(number), "{number}");
        }
        for name in ["x", "-", "+", "nil", "a1", "->x", "1x"] {
            assert!(!is_number(name), "{name}");
        }
    }
}
