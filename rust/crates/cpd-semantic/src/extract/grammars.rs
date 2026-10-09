//! Languages whose functions come from a tree-sitter grammar: C, C++, C#,
//! Go, Java, Kotlin, PHP, Ruby, Scala and Swift.
//!
//! Each language is a table — the grammar, the jscpd formats it serves and
//! the node kinds that are functions — read by one walker. The walker finds
//! where functions are and what they are called, which is what `--semantic`
//! needs.

use super::{FunctionExtractor, RawFunction};
use cpd_similarity::declared_name;
use cpd_tokenizer::line_index::LineIndex;
use tree_sitter::{Language, Node, Parser};
use tree_sitter_language::LanguageFn;

/// A language read through its tree-sitter grammar.
pub struct TreeSitterExtractor {
    grammar: &'static str,
    formats: &'static [&'static str],
    language: LanguageFn,
    /// Node kinds that are functions; nested ones are found too.
    functions: &'static [&'static str],
    /// How a function node shows that it has code.
    body: Body,
}

/// How a function node shows that it has a body. A node of a function kind
/// without one declares a function whose code is elsewhere or nowhere, such
/// as an interface or abstract method, a Go function written in assembly or
/// a C++ `= default`, so it takes no part.
#[derive(Clone, Copy)]
enum Body {
    /// The node's `body` field.
    Field,
    /// A child of this kind: Kotlin's `function_body` is not a field.
    Child(&'static str),
    /// Every node has one. An empty Ruby method has no `body` field, but it
    /// is a method all the same, as an empty JavaScript function is.
    Always,
}

/// Subtrees at the start of a function node that are not its own code:
/// annotations, attributes and comments. The span starts after them, as it
/// starts after a Python decorator or a Rust attribute.
const PREAMBLE: &[&str] = &[
    "annotation",
    "marker_annotation",
    "attribute",
    "attribute_list",
    "attribute_declaration",
    "attribute_specifier",
    "preproc_if_in_attribute_list",
    "comment",
    "line_comment",
    "block_comment",
    "multiline_comment",
];

pub static C: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "c",
    formats: &["c"],
    language: tree_sitter_c::LANGUAGE,
    functions: &["function_definition"],
    body: Body::Field,
};

pub static CPP: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "cpp",
    // `.h` files belong to C++ projects as often as to C ones, and the C++
    // grammar reads C headers too; the C grammar misreads C++ ones.
    formats: &["cpp", "cpp-header", "c-header"],
    language: tree_sitter_cpp::LANGUAGE,
    functions: &["function_definition", "lambda_expression"],
    body: Body::Field,
};

pub static CSHARP: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "csharp",
    formats: &["csharp"],
    language: tree_sitter_c_sharp::LANGUAGE,
    functions: &[
        "method_declaration",
        "constructor_declaration",
        "local_function_statement",
        "operator_declaration",
    ],
    body: Body::Field,
};

pub static GO: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "go",
    formats: &["go"],
    language: tree_sitter_go::LANGUAGE,
    functions: &["function_declaration", "method_declaration", "func_literal"],
    body: Body::Field,
};

pub static JAVA: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "java",
    formats: &["java"],
    language: tree_sitter_java::LANGUAGE,
    functions: &[
        "method_declaration",
        "constructor_declaration",
        "compact_constructor_declaration",
    ],
    body: Body::Field,
};

pub static KOTLIN: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "kotlin",
    formats: &["kotlin"],
    language: tree_sitter_kotlin_ng::LANGUAGE,
    functions: &["function_declaration", "anonymous_function"],
    body: Body::Child("function_body"),
};

pub static PHP: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "php",
    formats: &["php"],
    language: tree_sitter_php::LANGUAGE_PHP,
    functions: &[
        "function_definition",
        "method_declaration",
        "anonymous_function",
    ],
    body: Body::Field,
};

pub static RUBY: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "ruby",
    formats: &["ruby"],
    language: tree_sitter_ruby::LANGUAGE,
    functions: &["method", "singleton_method"],
    body: Body::Always,
};

pub static SCALA: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "scala",
    formats: &["scala"],
    language: tree_sitter_scala::LANGUAGE,
    functions: &["function_definition"],
    body: Body::Field,
};

pub static SWIFT: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "swift",
    formats: &["swift"],
    language: tree_sitter_swift::LANGUAGE,
    functions: &["function_declaration", "init_declaration"],
    body: Body::Field,
};

impl FunctionExtractor for TreeSitterExtractor {
    fn grammar(&self) -> &'static str {
        self.grammar
    }

    fn formats(&self) -> &'static [&'static str] {
        self.formats
    }

    fn extract(&self, source: &str, _format: &str) -> Vec<RawFunction> {
        let mut parser = Parser::new();
        if parser.set_language(&Language::new(self.language)).is_err() {
            return Vec::new();
        }
        // A grammar recovers from syntax errors; a tree comes back unless
        // parsing was cancelled, which nothing here does.
        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };
        let line_index = LineIndex::new(source.as_bytes());
        let mut out = Vec::new();
        let mut cursor = tree.walk();
        'walk: loop {
            let node = cursor.node();
            if node.is_named() && self.functions.contains(&node.kind()) && self.has_body(node) {
                // Its children are walked either way: a real function nested
                // in a recovered branch remains.
                if !recovered_branch(node, source) {
                    let start = line_index.location(code_start(node));
                    out.push(RawFunction {
                        grammar: self.grammar,
                        name: name_of(node, source),
                        head: start.clone(),
                        start,
                        end: line_index.location(node.end_byte()),
                        test: false,
                    });
                }
            }
            if cursor.goto_first_child() {
                continue;
            }
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    break 'walk;
                }
            }
        }
        out
    }
}

impl TreeSitterExtractor {
    fn has_body(&self, function: Node) -> bool {
        match self.body {
            Body::Field => function.child_by_field_name("body").is_some(),
            Body::Child(kind) => {
                let mut cursor = function.walk();
                function.children(&mut cursor).any(|c| c.kind() == kind)
            }
            Body::Always => true,
        }
    }
}

/// The byte where a function's own code starts: its first token outside
/// the annotations, attributes and comments it opens with.
fn code_start(function: Node) -> usize {
    let mut cursor = function.walk();
    'walk: loop {
        let node = cursor.node();
        let preamble = node.id() != function.id() && PREAMBLE.contains(&node.kind());
        if !preamble {
            if node.child_count() == 0 && node.end_byte() > node.start_byte() {
                return node.start_byte();
            }
            if cursor.goto_first_child() {
                continue;
            }
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                break 'walk;
            }
        }
    }
    function.start_byte()
}

/// What a function is called: its `name` field, the name inside a C or C++
/// declarator, or its node kind in angle brackets when it has none (a
/// closure, a lambda, an operator).
fn name_of(function: Node, source: &str) -> String {
    let named = function.child_by_field_name("name").or_else(|| {
        function
            .child_by_field_name("declarator")
            .and_then(declared_name)
    });
    match named {
        Some(node) => {
            let text = source.get(node.byte_range()).unwrap_or_default();
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        }
        None => format!("<{}>", function.kind()),
    }
}

/// Whether `function` is an `else` branch that error recovery read as a
/// function (#1163). Around a `#if` or `#ifdef`, the C, C++ and C# grammars
/// turn `else if (…) { }` into a function whose type is `else` and whose
/// name is `if`, and the same for `else while`, `else using`, `else await
/// using` and the like. `else` is a keyword in every language here, so no
/// real function has it for a type.
fn recovered_branch(function: Node, source: &str) -> bool {
    function
        .child_by_field_name("type")
        .and_then(|kind| source.get(kind.byte_range()))
        == Some("else")
}

#[cfg(test)]
mod tests {
    use crate::extract::extract_functions;
    use crate::units::supports_units;

    /// Name, first line and last line of each function, and the source
    /// from where each one starts, cut to `width` bytes.
    fn found(src: &str, format: &str, width: usize) -> Vec<(String, u32, u32, String)> {
        extract_functions(src, format)
            .into_iter()
            .map(|f| {
                let from = &src[f.start.offset as usize..];
                let head = from.get(..width.min(from.len())).unwrap_or(from);
                (f.name, f.start.line, f.end.line, head.to_string())
            })
            .collect()
    }

    fn row(name: &str, first: u32, last: u32, head: &str) -> (String, u32, u32, String) {
        (name.to_string(), first, last, head.to_string())
    }

    #[test]
    fn go_functions_methods_and_literals() {
        let src = "package main\n\n// Add adds.\nfunc Add(a, b int) int {\n\treturn a + b\n}\n\nfunc (c *Cart) Total() int {\n\tsum := func(xs []int) int { return len(xs) }\n\treturn sum(c.items)\n}\n";
        assert_eq!(
            found(src, "go", 6),
            vec![
                row("Add", 4, 6, "func A"),
                row("Total", 8, 11, "func ("),
                row("<func_literal>", 9, 9, "func(x"),
            ]
        );
    }

    #[test]
    fn java_methods_start_after_their_annotations() {
        let src = "class Cart {\n    @Override\n    public int total() {\n        return items.stream().mapToInt(i -> i.price).sum();\n    }\n\n    Cart(List<Item> items) { this.items = items; }\n}\n";
        assert_eq!(
            found(src, "java", 10),
            vec![
                row("total", 3, 5, "public int"),
                row("Cart", 7, 7, "Cart(List<"),
            ]
        );
    }

    /// Issue #1163: `else if` after `#if … #endif` was recovered as a
    /// function named `if`. The file holds one function, `Describe`.
    #[test]
    fn csharp_else_if_after_a_directive_is_not_a_function() {
        let src = r#"public class Keys
{
    public static string Describe(object key)
    {
        if (key is int i)
        {
            return "int " + i;
        }
#if FEATURE_LONG
        else if (key is long l)
        {
            return "long " + l;
        }
#endif
        else if (key is short s)
        {
            return "short " + s;
        }
        return "other";
    }
}
"#;
        let fns = extract_functions(src, "csharp");
        let names: Vec<&str> = fns.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Describe"]);
        let start = src.find("public static string Describe").unwrap();
        let marker = "return \"other\";\n    }";
        let end = start + src[start..].find(marker).unwrap() + marker.len();
        let describe = &fns[0];
        assert_eq!(describe.start.offset as usize, start);
        assert_eq!(describe.end.offset as usize, end);
        assert_eq!((describe.start.line, describe.start.column), (3, 4));
        assert_eq!((describe.end.line, describe.end.column), (20, 5));
        assert_eq!(
            &src[start..end],
            &src[describe.start.offset as usize..describe.end.offset as usize]
        );
    }

    /// Several conditional-compilation branches, including a directive
    /// nested in the next method. Branch bodies are not functions; the
    /// methods are, and so is a local function written inside a branch.
    #[test]
    fn csharp_directive_branches_are_not_functions() {
        let src = r#"public class Units
{
    public static string Label(object value)
    {
        if (value is double d)
        {
            return $"{d} m";
        }
#if METRIC
        else if (value is float f)
        {
            return $"{f} cm";
        }
#elif IMPERIAL
        else if (value is decimal m)
        {
            int Nested()
            {
                return (int)m;
            }
            return $"{Nested()} in";
        }
#else
        else if (value is uint u)
        {
            return $"{u} ft";
        }
#endif
        return "?";
    }

    public static string After()
    {
#if OUTER
#if INNER
        if (true)
        {
            return "a";
        }
#else
        else if (false)
        {
            return "b";
        }
#endif
#else
        return "c";
#endif
    }

    int Next()
    {
        return 1;
    }
}
"#;
        let names: Vec<String> = extract_functions(src, "csharp")
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(names, ["Label", "Nested", "After", "Next"]);
    }

    /// The C and C++ grammars recover an `else` branch after `#ifdef` the
    /// same way, and C# does so for every statement after `else`, a
    /// contextual keyword such as `await` too.
    #[test]
    fn else_branches_recovered_as_functions_are_dropped() {
        let cases: &[(&str, &str, &[&str])] = &[
            (
                "c",
                "int pick(int k) {\n  if (k == 1) { return 1; }\n#ifdef WIDE\n  else if (k == 2) { return 2; }\n#endif\n  else if (k == 3) { return 3; }\n  return 0;\n}\n",
                &["pick"],
            ),
            (
                "c",
                "int spin(int k) {\n  if (k > 9) { return 9; }\n#ifdef LOOP\n  else if (k > 5) { return 5; }\n#endif\n  else while (k > 2) { k--; }\n  return k;\n}\n",
                &["spin"],
            ),
            (
                "cpp",
                "void Runner::run() {\n  if (ready) { start(); }\n#if FAST\n  else if (warm) { resume(); }\n#endif\n  else { stop(); }\n}\n",
                &["run"],
            ),
            (
                "csharp",
                "class Io {\n  async Task Read() {\n    if (cached) { Use(); }\n#if NET8\n    else if (fresh) { Load(); }\n#endif\n    else await using (var r = Open()) { Take(r); }\n  }\n}\n",
                &["Read"],
            ),
        ];
        for (grammar, src, expected) in cases {
            let names: Vec<String> = extract_functions(src, grammar)
                .into_iter()
                .map(|f| f.name)
                .collect();
            assert_eq!(&names, expected, "{grammar}: {src}");
        }
    }

    /// Contextual keywords and a verbatim `@if` are real names, as are a
    /// constructor, an operator and a local function. A method stays when a
    /// descendant does not parse, and so does the method after it.
    #[test]
    fn csharp_keeps_contextual_verbatim_and_broken_methods() {
        let src = r#"public class Names
{
    public int @if() { return 1; }
    public int var() { return 1; }
    public int from() { return 1; }
    public int when() { return 1; }
    public int await() { return 1; }
    public int record() { return 1; }
    public Names() { }
    public static Names operator +(Names a, Names b) { return a; }
    public int Local()
    {
        int Twice(int x) { return x * 2; }
        return Twice(1);
    }
}
"#;
        let names: Vec<String> = extract_functions(src, "csharp")
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(
            names,
            [
                "@if",
                "var",
                "from",
                "when",
                "await",
                "record",
                "Names",
                "<operator_declaration>",
                "Local",
                "Twice",
            ]
        );

        let src = r#"public class Box
{
    public int Ok()
    {
        int x = ;
        return 1;
    }

    public int Also() { return 2; }
}

public void Broken( {
"#;
        let fns = extract_functions(src, "csharp");
        let names: Vec<&str> = fns.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Ok", "Also"]);
        let ok = &src[fns[0].start.offset as usize..fns[0].end.offset as usize];
        assert!(ok.contains("int x = ;"), "{ok}");
    }

    #[test]
    fn csharp_methods_constructors_and_local_functions() {
        let src = "public class Cart {\n    [Obsolete(\"x\")]\n    public int Total() => items.Sum(i => i.Price);\n\n    public Cart() { }\n\n    int Local() {\n        int Twice(int x) { return x * 2; }\n        return Twice(2);\n    }\n}\n";
        assert_eq!(
            found(src, "csharp", 10),
            vec![
                row("Total", 3, 3, "public int"),
                row("Cart", 5, 5, "public Car"),
                row("Local", 7, 10, "int Local("),
                row("Twice", 8, 8, "int Twice("),
            ]
        );
    }

    #[test]
    fn kotlin_functions_with_block_and_expression_bodies() {
        let src = "class Cart {\n    @JvmStatic\n    fun total(items: List<Int>): Int = items.sum()\n\n    private fun label(): String {\n        return \"cart\"\n    }\n}\n";
        assert_eq!(
            found(src, "kotlin", 9),
            vec![
                row("total", 3, 3, "fun total"),
                row("label", 5, 7, "private f"),
            ]
        );
    }

    #[test]
    fn php_functions_and_methods_after_attributes() {
        let src = "<?php\nfunction add($a, $b) {\n    return $a + $b;\n}\nclass Cart {\n    #[Pure]\n    public function total(): int {\n        return array_sum($this->items);\n    }\n}\n";
        assert_eq!(
            found(src, "php", 12),
            vec![
                row("add", 2, 4, "function add"),
                row("total", 7, 9, "public funct"),
            ]
        );
    }

    #[test]
    fn ruby_methods_and_singleton_methods() {
        let src = "class Cart\n  def total\n    items.sum(&:price)\n  end\n\n  def self.empty?\n    true\n  end\nend\n";
        assert_eq!(
            found(src, "ruby", 9),
            vec![
                row("total", 2, 4, "def total"),
                row("empty?", 6, 8, "def self."),
            ]
        );
    }

    #[test]
    fn c_functions_are_named_through_their_declarators() {
        let src = "static int add(int a, int b) {\n    return a + b;\n}\n\nchar *name(void) { return \"x\"; }\n";
        assert_eq!(
            found(src, "c", 10),
            vec![
                row("add", 1, 3, "static int"),
                row("name", 5, 5, "char *name"),
            ]
        );
    }

    #[test]
    fn cpp_qualified_names_destructors_and_lambdas() {
        let src = "namespace shop {\nint Cart::total() const {\n    auto twice = [](int x) { return x * 2; };\n    return twice(sum);\n}\n}\nclass Box {\n    ~Box() {}\n};\n";
        assert_eq!(
            found(src, "cpp", 8),
            vec![
                row("total", 2, 5, "int Cart"),
                row("<lambda_expression>", 3, 3, "[](int x"),
                row("~Box", 8, 8, "~Box() {"),
            ]
        );
    }

    #[test]
    fn cpp_names_survive_a_macro_before_the_return_type() {
        let src = "template<typename Value>\nJSON_RETURNS_NON_NULL\nBasicJsonType* handle_value(Value&& v)\n{\n    return nullptr;\n}\n";
        let names: Vec<String> = extract_functions(src, "cpp")
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(names, vec!["handle_value"]);
    }

    #[test]
    fn c_headers_are_read_with_the_cpp_grammar() {
        let src = "namespace fuzzer {\nstruct Map {\n  inline bool Get(int i) {\n    return bits[i];\n  }\n};\n}\n";
        let found: Vec<(String, &str)> = extract_functions(src, "c-header")
            .into_iter()
            .map(|f| (f.name, f.grammar))
            .collect();
        assert_eq!(found, vec![("Get".to_string(), "cpp")]);
    }

    #[test]
    fn swift_functions_and_initializers_after_attributes() {
        let src = "struct Cart {\n    @discardableResult\n    func total() -> Int {\n        return items.reduce(0, +)\n    }\n    init(items: [Int]) { self.items = items }\n}\n";
        assert_eq!(
            found(src, "swift", 10),
            vec![
                row("total", 3, 5, "func total"),
                row("init", 6, 6, "init(items"),
            ]
        );
    }

    #[test]
    fn scala_definitions_with_and_without_blocks() {
        let src = "object Cart {\n  @inline def total(xs: List[Int]): Int = xs.sum\n  def label: String = {\n    \"cart\"\n  }\n}\n";
        assert_eq!(
            found(src, "scala", 9),
            vec![
                row("total", 2, 2, "def total"),
                row("label", 3, 5, "def label"),
            ]
        );
    }

    #[test]
    fn declarations_without_a_body_take_no_part() {
        let cases: &[(&str, &str, &[&str])] = &[
            (
                "java",
                "interface Shape {\n    double area();\n    default String name() { return \"s\"; }\n}\nabstract class Base {\n    abstract void run();\n    native int fast();\n    void go() { }\n}\n",
                &["name", "go"],
            ),
            (
                "csharp",
                "interface IShape {\n    double Area();\n    string Name() => \"s\";\n    static abstract IShape operator +(IShape a, IShape b);\n}\nabstract class Base {\n    public abstract void Run();\n    extern static int Fast();\n    partial void Hook();\n    public int Total() { return 1; }\n}\n",
                &["Name", "Total"],
            ),
            (
                "go",
                "package p\n\nfunc asm(a int) int\n\nfunc real(a int) int { return a }\n",
                &["real"],
            ),
            (
                "kotlin",
                "interface Shape {\n    fun area(): Double\n    fun name(): String = \"s\"\n}\nabstract class Base {\n    abstract fun run()\n    external fun fast(a: Int): Int\n    fun go() {\n    }\n}\n",
                &["name", "go"],
            ),
            (
                "php",
                "<?php\ninterface Shape {\n    public function area(): float;\n}\nabstract class Base {\n    abstract protected function run();\n    public function go() { return 1; }\n}\n",
                &["go"],
            ),
            (
                "cpp",
                "struct A {\n    A() = default;\n    A(const A&) = delete;\n    virtual void f() = 0;\n    void g() { }\n};\n",
                &["g"],
            ),
            (
                "swift",
                "protocol Shape {\n    init(size: Int)\n    func area() -> Double\n}\nstruct Box {\n    init(size: Int) { }\n}\n",
                &["init"],
            ),
            ("ruby", "class Cart\n  def clear\n  end\nend\n", &["clear"]),
        ];
        for (format, src, expected) in cases {
            let names: Vec<String> = extract_functions(src, format)
                .into_iter()
                .map(|f| f.name)
                .collect();
            assert_eq!(names, *expected, "{format}");
        }
    }

    #[test]
    fn grammar_languages_serve_semantic_units_but_not_similarity() {
        for format in [
            "c",
            "c-header",
            "cpp",
            "cpp-header",
            "csharp",
            "go",
            "java",
            "kotlin",
            "php",
            "ruby",
            "scala",
            "swift",
        ] {
            assert!(supports_units(format), "{format}");
        }
    }

    #[test]
    fn broken_sources_still_yield_what_parses() {
        let src = "func ok() int {\n\treturn 1\n}\n\nfunc broken( {\n";
        let names: Vec<String> = extract_functions(src, "go")
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert!(names.contains(&"ok".to_string()), "{names:?}");
    }
}
