//! Clojure: every top-level form except `ns` is a unit.
//!
//! A small reader turns the source into forms; it evaluates nothing. Reader
//! conditionals keep their `:clj` branch (or `:default`), `#_` drops the next
//! form, metadata drops out, and quote, syntax-quote, unquote, deref and var
//! quote become lists headed by their names, as `quote` and `deref` are in
//! Clojure itself. A form the reader cannot read ends the file: the forms
//! before it still count.
//!
//! A unit starts at its own opening bracket, inside a reader conditional
//! too, and ends where its last part ends: a comment, a dropped form or a
//! closing bracket on a line of its own after it does not make the unit
//! longer. Code that `jscpd:ignore` or `--ignore-pattern` skips inside a
//! form adds nothing to it.
//!
//! Normalized, a symbol is `:symbol` unless it heads a list, where it stays
//! as `[:symbol "map"]`; a keyword is `:keyword`, every other literal
//! `:literal`, and a map holds `[key value]` pairs.

use crate::prints::{Prints, Value, keyword};

/// A form read from Clojure source: its shape and the bytes it spans.
#[derive(Debug)]
struct Form {
    shape: Shape,
    start: usize,
    end: usize,
}

#[derive(Debug)]
enum Shape {
    Symbol(String),
    /// A keyword, by its name without the colons.
    Keyword(String),
    Literal,
    Coll(Kind, Vec<Form>),
}

impl Form {
    fn atom(shape: Shape, start: usize, end: usize) -> Self {
        Self { shape, start, end }
    }

    fn coll(kind: Kind, items: Vec<Form>, start: usize, end: usize) -> Self {
        Self {
            shape: Shape::Coll(kind, items),
            start,
            end,
        }
    }
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
        let Ok(form) = reader.read() else {
            break;
        };
        let Some(form) = form else {
            continue;
        };
        let Shape::Coll(Kind::List, items) = &form.shape else {
            continue;
        };
        let Some(head) = items.first() else {
            continue;
        };
        if matches!(&head.shape, Shape::Symbol(name) if name == "ns") {
            continue;
        }
        // Where the last part ends, and the closing bracket with it when it
        // is on the same line.
        let last = items.last().map_or(form.end, |item| item.end);
        let line_end = source.as_bytes()[last..]
            .iter()
            .position(|&byte| byte == b'\n')
            .map_or(source.len(), |at| last + at);
        let (start, end) = (form.start, form.end.min(line_end));
        if crate::syntax::inside(ignored, start, end) {
            continue;
        }
        let name = match items.get(1).map(|item| &item.shape) {
            Some(Shape::Symbol(name)) => name.clone(),
            _ => match &head.shape {
                Shape::Symbol(name) => name.clone(),
                _ => "<form>".to_string(),
            },
        };
        let mut prints = Prints::default();
        let root = normalize(&form, ignored, &mut prints);
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

/// The normalized tree of `form` without the parts inside the sorted,
/// disjoint byte ranges `ignored`, children before parents with a stack of
/// its own: deeply nested data must not overflow the thread's stack.
fn normalize(form: &Form, ignored: &[[usize; 2]], prints: &mut Prints) -> Value {
    enum Step<'f> {
        Enter(&'f Form, bool),
        /// A collection and how many of its parts were read.
        Leave(&'f Form, usize),
    }
    let skipped = |item: &Form| crate::syntax::inside(ignored, item.start, item.end);
    let mut values: Vec<Value> = Vec::new();
    let mut stack = vec![Step::Enter(form, false)];
    while let Some(step) = stack.pop() {
        match step {
            Step::Enter(form, head) => match &form.shape {
                Shape::Symbol(name) => values.push(match head {
                    true => prints.symbol(name),
                    false => Value::Atom(keyword("symbol")),
                }),
                Shape::Keyword(..) => values.push(Value::Atom(keyword("keyword"))),
                Shape::Literal => values.push(Value::Atom(keyword("literal"))),
                Shape::Coll(kind, items) => {
                    // A map loses a pair a skipped key or value is in.
                    let kept: Vec<(usize, &Form)> = match kind {
                        Kind::Map => items
                            .chunks(2)
                            .enumerate()
                            .filter(|(_, pair)| !pair.iter().any(&skipped))
                            .flat_map(|(n, pair)| {
                                pair.iter()
                                    .enumerate()
                                    .map(move |(k, item)| (2 * n + k, item))
                            })
                            .collect(),
                        _ => items
                            .iter()
                            .enumerate()
                            .filter(|(_, item)| !skipped(item))
                            .collect(),
                    };
                    stack.push(Step::Leave(form, kept.len()));
                    for &(i, item) in kept.iter().rev() {
                        stack.push(Step::Enter(item, *kind == Kind::List && i == 0));
                    }
                }
            },
            Step::Leave(form, read) => {
                let Shape::Coll(kind, _) = &form.shape else {
                    unreachable!("only collections are left");
                };
                let children = values.split_off(values.len() - read);
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

/// What a form being read is inside of, with the byte where it starts.
enum Open {
    /// A collection, with the byte that closes it.
    Coll(Kind, Vec<Form>, u8, usize),
    /// `#(…)`: a list headed by `fn*`.
    Function(Vec<Form>, usize),
    /// Quote and the like: a list of the name and the next form.
    Wrap(&'static str, usize),
    /// `#_`: the next form drops out.
    Discard,
    /// `^`: the next form is metadata and drops out, the one after it stays.
    Meta,
    /// A tagged literal: the next form is its value.
    Tagged(usize),
    /// A reader conditional, splicing or not: the next form is its body.
    Conditional(bool),
    /// A cljx feature expression, `#+` or `#-`: the next form names the
    /// feature, and the form after it stays or drops out.
    Feature(bool),
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
        let text = source.as_bytes();
        let at = match text.starts_with("\u{feff}".as_bytes()) {
            true => 3,
            false => 0,
        };
        let mut reader = Self { text, at };
        if text[at..].starts_with(b"#!") {
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
                    Open::Coll(_, items, _, _) | Open::Function(items, _) => {
                        match read {
                            Read::Form(form) => items.push(form),
                            Read::Splice(forms) => items.extend(forms),
                            Read::Nothing => {}
                        }
                        break;
                    }
                    // A form that reads as nothing, like the one `#_` drops,
                    // leaves a prefix waiting for the next one: `#_#_ a b`
                    // drops both forms, as Clojure does.
                    _ if matches!(read, Read::Nothing) => break,
                    Open::Wrap(name, start) => {
                        let (name, start) = (*name, *start);
                        open.pop();
                        let mut items =
                            vec![Form::atom(Shape::Symbol(name.to_string()), start, start)];
                        if let Read::Form(form) = read {
                            items.push(form);
                        }
                        read = Read::Form(Form::coll(Kind::List, items, start, self.at));
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
                    Open::Feature(plus) => {
                        // `#+clj form` keeps the form, as Clojure on the JVM
                        // reads it, `#+cljs form` drops it, `#-` the other
                        // way round.
                        let plus = *plus;
                        open.pop();
                        let clj = matches!(&read, Read::Form(form) if mentions_clj(form));
                        if plus != clj {
                            open.push(Open::Discard);
                        }
                        break;
                    }
                    Open::Tagged(start) => {
                        let start = *start;
                        open.pop();
                        read = Read::Form(Form::atom(Shape::Literal, start, self.at));
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
                Some(Open::Coll(_, _, close, _)) => Some(*close),
                Some(Open::Function(..)) => Some(b')'),
                _ => None,
            };
            if close == Some(byte) {
                self.bump();
                return Ok(Read::Form(close_collection(open.pop(), self.at)?));
            }
            let start = self.at;
            match byte {
                b'(' | b'[' | b'{' => {
                    self.bump();
                    let (kind, close) = match byte {
                        b'(' => (Kind::List, b')'),
                        b'[' => (Kind::Vector, b']'),
                        _ => (Kind::Map, b'}'),
                    };
                    open.push(Open::Coll(kind, Vec::new(), close, start));
                }
                b')' | b']' | b'}' => return Err(Unreadable),
                b'"' => {
                    self.string()?;
                    return Ok(Read::Form(Form::atom(Shape::Literal, start, self.at)));
                }
                b'\\' => {
                    self.bump();
                    self.bump().ok_or(Unreadable)?;
                    self.token();
                    return Ok(Read::Form(Form::atom(Shape::Literal, start, self.at)));
                }
                b':' => {
                    self.bump();
                    if self.peek() == Some(b':') {
                        self.bump();
                    }
                    let name = self.token().to_string();
                    return Ok(Read::Form(Form::atom(Shape::Keyword(name), start, self.at)));
                }
                b'\'' | b'@' | b'`' => {
                    self.bump();
                    let name = match byte {
                        b'\'' => "quote",
                        b'@' => "deref",
                        _ => "syntax-quote",
                    };
                    open.push(Open::Wrap(name, start));
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
                    open.push(Open::Wrap(name, start));
                }
                b'^' => {
                    self.bump();
                    open.push(Open::Meta);
                }
                b'#' => {
                    self.bump();
                    if let Some(read) = self.dispatch(open, start)? {
                        return Ok(read);
                    }
                }
                _ => {
                    let token = self.token();
                    if token.is_empty() {
                        return Err(Unreadable);
                    }
                    let shape = match is_number(token) {
                        true => Shape::Literal,
                        false => Shape::Symbol(token.to_string()),
                    };
                    return Ok(Read::Form(Form::atom(shape, start, self.at)));
                }
            }
        }
    }

    /// What follows the `#` at `start`: a form when it reads one at once,
    /// `None` when it opened something.
    fn dispatch(&mut self, open: &mut Vec<Open>, start: usize) -> Result<Option<Read>, Unreadable> {
        let byte = self.peek().ok_or(Unreadable)?;
        match byte {
            b'_' => {
                self.bump();
                open.push(Open::Discard);
            }
            b'{' => {
                self.bump();
                open.push(Open::Coll(Kind::Set, Vec::new(), b'}', start));
            }
            b'(' => {
                self.bump();
                open.push(Open::Function(Vec::new(), start));
            }
            b'"' => {
                self.string()?;
                return Ok(Some(Read::Form(Form::atom(Shape::Literal, start, self.at))));
            }
            b'\'' => {
                self.bump();
                open.push(Open::Wrap("var", start));
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
                return Ok(Some(Read::Form(Form::atom(Shape::Literal, start, self.at))));
            }
            b'=' => {
                self.bump();
                open.push(Open::Tagged(start));
            }
            b'+' | b'-' => {
                self.bump();
                open.push(Open::Feature(byte == b'+'));
            }
            _ => {
                if self.token().is_empty() {
                    return Err(Unreadable);
                }
                open.push(Open::Tagged(start));
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

/// The collection `open` closed by the bracket that ends at `end`.
fn close_collection(open: Option<Open>, end: usize) -> Result<Form, Unreadable> {
    match open {
        Some(Open::Coll(Kind::Map, items, _, _)) if items.len() % 2 == 1 => Err(Unreadable),
        Some(Open::Coll(kind, items, _, start)) => Ok(Form::coll(kind, items, start, end)),
        // `#(f a b)` is `(fn* [] (f a b))`: the call in it heads a list.
        Some(Open::Function(items, start)) => {
            let all = vec![
                Form::atom(Shape::Symbol("fn*".to_string()), start, start),
                Form::coll(Kind::Vector, Vec::new(), start, start),
                Form::coll(Kind::List, items, start, end),
            ];
            Ok(Form::coll(Kind::List, all, start, end))
        }
        _ => Err(Unreadable),
    }
}

/// The branch of a reader conditional that Clojure on the JVM reads: the
/// `:clj` one, or `:default`; spliced into the collection around for `#?@`.
fn conditional(body: Read, splicing: bool) -> Result<Read, Unreadable> {
    let Read::Form(Form {
        shape: Shape::Coll(Kind::List, items),
        ..
    }) = body
    else {
        return Err(Unreadable);
    };
    let branch = items.chunks(2).position(|pair| {
        matches!(pair, [Form { shape: Shape::Keyword(name), .. }, _] if name == "clj" || name == "default")
    });
    let Some(chosen) = branch.and_then(|at| items.into_iter().nth(at * 2 + 1)) else {
        return Ok(Read::Nothing);
    };
    if !splicing {
        return Ok(Read::Form(chosen));
    }
    match chosen.shape {
        Shape::Coll(Kind::List | Kind::Vector, items) => Ok(Read::Splice(items)),
        _ => Err(Unreadable),
    }
}

/// Whether a cljx feature names Clojure on the JVM: `clj`, or a list such as
/// `(or clj cljs)` with it.
fn mentions_clj(form: &Form) -> bool {
    let clj = |form: &Form| matches!(&form.shape, Shape::Symbol(name) if name == "clj");
    match &form.shape {
        Shape::Coll(_, items) => items.iter().any(clj),
        _ => clj(form),
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
    fn two_discards_drop_two_forms() {
        let source =
            "(defn a [x]\n  {#_#_ :b 2 :c x})\n#_#_(defn b [] 1) (defn c [] 2)\n(defn d [] 3)\n";
        let units = forms(source, &[]);
        let names: Vec<&str> = units.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["a", "d"]);
    }

    #[test]
    fn a_unit_spans_its_own_lines() {
        // Inside a reader conditional a unit starts at its own bracket; a
        // comment, a dropped form and a lone closing bracket after its last
        // part do not make it longer.
        let source = "#?(:clj\n   (defn a [x]\n     (inc x))\n   :cljs\n   (defn a [x] x))\n(defn b [x]\n  (inc x)\n  ;; done\n  #_(dec x)\n  )\n";
        let line = |byte: usize| source[..byte].matches('\n').count() + 1;
        let lines: Vec<(usize, usize)> = forms(source, &[])
            .iter()
            .map(|unit| (line(unit.start_byte), line(unit.end_byte)))
            .collect();
        assert_eq!(lines, [(2, 3), (6, 7)]);
    }

    #[test]
    fn a_byte_order_mark_keeps_the_offsets() {
        let source = "\u{feff}(defn a [x]\n  (inc x))\n";
        let units = forms(source, &[]);
        assert_eq!(
            &source[units[0].start_byte..units[0].end_byte],
            "(defn a [x]\n  (inc x))"
        );
    }

    #[test]
    fn a_function_literal_keeps_the_call_in_it() {
        let adults = forms(
            "(defn a [people]\n  (filter #(>= (:age %) 18) people))\n",
            &[],
        );
        let minors = forms(
            "(defn b [people]\n  (filter #(< (:age %) 18) people))\n",
            &[],
        );
        assert!(jaccard(&adults[0].fingerprints, &minors[0].fingerprints) < 1.0);
    }

    #[test]
    fn ignored_code_inside_a_form_adds_nothing() {
        let source = "(defn a [x]\n  (inc x)\n  (log x))\n";
        let at = source.find("(log x)").unwrap();
        let with = forms(source, &[[at, at + "(log x)".len()]]);
        let without = forms("(defn a [x]\n  (inc x))\n", &[]);
        assert_eq!(with[0].fingerprints, without[0].fingerprints);
    }

    #[test]
    fn a_unit_ends_where_its_last_part_ends() {
        let source = "(def doc\n  \"first\n  second\n  third\")\n";
        let units = forms(source, &[]);
        let line = |byte: usize| source[..byte].matches('\n').count() + 1;
        assert_eq!(line(units[0].end_byte), 4);
    }

    #[test]
    fn cljx_feature_expressions_keep_the_forms_for_clj() {
        let source = "(defn a [x]\n  #+clj (inc x)\n  #+cljs (dec x)\n  #-clj (log x))\n";
        let units = forms(source, &[]);
        let plain = forms("(defn a [x]\n  (inc x))\n", &[]);
        assert_eq!(units[0].fingerprints, plain[0].fingerprints);
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
