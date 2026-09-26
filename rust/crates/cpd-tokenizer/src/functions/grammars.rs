//! Languages whose functions come from a tree-sitter grammar: C, C++, C#,
//! Go, Java, Kotlin, PHP, Ruby, Scala and Swift.
//!
//! Each language is a table — the grammar, the jscpd formats it serves and
//! the node kinds that are functions — read by one walker. The walker finds
//! where functions are and what they are called, which is what `--semantic`
//! needs; `kinds` stays empty, so `--similarity` does not use these
//! languages (see [`FunctionExtractor::structural`]).

use super::{FunctionExtractor, RawFunction};
use crate::line_index::LineIndex;
use tree_sitter::{Language, Node, Parser};
use tree_sitter_language::LanguageFn;

/// A language read through its tree-sitter grammar.
pub struct TreeSitterExtractor {
    grammar: &'static str,
    formats: &'static [&'static str],
    language: LanguageFn,
    /// Node kinds that are functions; nested ones are found too.
    functions: &'static [&'static str],
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

/// Nodes that are a C or C++ function's name, at the end of its declarator.
/// A `type_identifier` is one when a macro before the return type makes the
/// grammar read the real type as a scope.
const NAMES: &[&str] = &[
    "identifier",
    "field_identifier",
    "type_identifier",
    "destructor_name",
    "operator_name",
];

/// Declarator wrappers around a C or C++ function's name.
const DECLARATORS: &[&str] = &[
    "function_declarator",
    "pointer_declarator",
    "pointer_type_declarator",
    "reference_declarator",
    "attributed_declarator",
    "parenthesized_declarator",
];

/// Qualified or templated names; the name proper is their `name` field.
const QUALIFIED: &[&str] = &[
    "qualified_identifier",
    "template_function",
    "template_method",
];

pub static C: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "c",
    formats: &["c"],
    language: tree_sitter_c::LANGUAGE,
    functions: &["function_definition"],
};

pub static CPP: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "cpp",
    // `.h` files belong to C++ projects as often as to C ones, and the C++
    // grammar reads C headers too; the C grammar misreads C++ ones.
    formats: &["cpp", "cpp-header", "c-header"],
    language: tree_sitter_cpp::LANGUAGE,
    functions: &["function_definition", "lambda_expression"],
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
};

pub static GO: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "go",
    formats: &["go"],
    language: tree_sitter_go::LANGUAGE,
    functions: &["function_declaration", "method_declaration", "func_literal"],
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
};

pub static KOTLIN: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "kotlin",
    formats: &["kotlin"],
    language: tree_sitter_kotlin_ng::LANGUAGE,
    functions: &["function_declaration", "anonymous_function"],
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
};

pub static RUBY: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "ruby",
    formats: &["ruby"],
    language: tree_sitter_ruby::LANGUAGE,
    functions: &["method", "singleton_method"],
};

pub static SCALA: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "scala",
    formats: &["scala"],
    language: tree_sitter_scala::LANGUAGE,
    functions: &["function_definition"],
};

pub static SWIFT: TreeSitterExtractor = TreeSitterExtractor {
    grammar: "swift",
    formats: &["swift"],
    language: tree_sitter_swift::LANGUAGE,
    functions: &["function_declaration", "init_declaration"],
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
            if node.is_named() && self.functions.contains(&node.kind()) {
                let start = line_index.location(code_start(node));
                out.push(RawFunction {
                    grammar: self.grammar,
                    name: name_of(node, source),
                    head: start.clone(),
                    start,
                    end: line_index.location(node.end_byte()),
                    kinds: Vec::new(),
                });
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

    fn structural(&self) -> bool {
        false
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

/// The name a C or C++ declarator wraps: `f` in `*f(void)`, `(&f)(int)` or
/// `shop::Cart::f()`. None for a declarator that names nothing, such as a
/// lambda's parameter list.
fn declared_name(mut node: Node) -> Option<Node> {
    loop {
        let kind = node.kind();
        if NAMES.contains(&kind) {
            return Some(node);
        }
        node = if DECLARATORS.contains(&kind) {
            match node.child_by_field_name("declarator") {
                Some(inner) => inner,
                None => {
                    let last = node.named_child_count().checked_sub(1)?;
                    node.named_child(u32::try_from(last).ok()?)?
                }
            }
        } else if QUALIFIED.contains(&kind) {
            node.child_by_field_name("name")?
        } else {
            return None;
        };
    }
}

#[cfg(test)]
mod tests {
    use crate::functions::{extract_functions, supports_functions};
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
            assert!(!supports_functions(format), "{format}");
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
