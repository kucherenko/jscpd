//! Functions and methods of the languages read through a tree-sitter
//! grammar: Python, JavaScript and TypeScript, Java, Kotlin, Scala, C#, Go,
//! Rust, C, C++, PHP, Ruby and Swift.
//!
//! Each language picks the nodes that are units ([`Grammar::units`]), and
//! one normalizer turns a unit's syntax tree into the tree whose subtrees
//! are its fingerprints ([`crate::prints`]):
//!
//! - a named node is a list: a keyword with its type, then its children;
//! - punctuation, the language's keywords and comments drop out;
//! - an operator stays as `[:symbol "+"]`;
//! - a name becomes `:symbol` and a literal `:literal`, except the name a
//!   call invokes, which stays as `[:symbol "map"]`: `filter(xs)` and
//!   `map(xs)` differ while `total` and `sum` do not. A method keeps its
//!   name and loses its receiver's, a path such as `std::mem::swap` keeps
//!   every name in it;
//! - parentheses around one expression drop out.
//!
//! Grammars name these nodes differently: [`Tables`] holds, per grammar,
//! the names it adds to the ones most grammars share.

use crate::prints::{Prints, Value, keyword};
use rustc_hash::FxHashMap;
use tree_sitter::{Language, Node, Parser};
use tree_sitter_language::LanguageFn;

/// A grammar `--similarity` reads, by the jscpd formats it serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grammar {
    Python,
    TypeScript,
    /// TypeScript with JSX, which also reads JavaScript and JSX.
    Tsx,
    Java,
    Go,
    Rust,
    Kotlin,
    Scala,
    CSharp,
    C,
    /// C++, and the `.h` headers, which the C++ grammar reads for C too.
    Cpp,
    Php,
    Ruby,
    Swift,
}

impl Grammar {
    pub(crate) fn for_format(format: &str) -> Option<Self> {
        Some(match format {
            "python" => Self::Python,
            "typescript" => Self::TypeScript,
            "tsx" | "javascript" | "jsx" => Self::Tsx,
            "java" => Self::Java,
            "go" => Self::Go,
            "rust" => Self::Rust,
            "kotlin" => Self::Kotlin,
            "scala" => Self::Scala,
            "csharp" => Self::CSharp,
            "c" => Self::C,
            "cpp" | "cpp-header" | "c-header" => Self::Cpp,
            "php" => Self::Php,
            "ruby" => Self::Ruby,
            "swift" => Self::Swift,
            _ => return None,
        })
    }

    fn language(self) -> LanguageFn {
        match self {
            Self::Python => tree_sitter_python::LANGUAGE,
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
            Self::Java => tree_sitter_java::LANGUAGE,
            Self::Go => tree_sitter_go::LANGUAGE,
            Self::Rust => tree_sitter_rust::LANGUAGE,
            Self::Kotlin => tree_sitter_kotlin_ng::LANGUAGE,
            Self::Scala => tree_sitter_scala::LANGUAGE,
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE,
            Self::C => tree_sitter_c::LANGUAGE,
            Self::Cpp => tree_sitter_cpp::LANGUAGE,
            Self::Php => tree_sitter_php::LANGUAGE_PHP,
            Self::Ruby => tree_sitter_ruby::LANGUAGE,
            Self::Swift => tree_sitter_swift::LANGUAGE,
        }
    }

    fn tables(self) -> &'static Tables {
        match self {
            Self::Python | Self::TypeScript | Self::Tsx | Self::Java | Self::Go | Self::Rust => {
                &SHARED
            }
            Self::Kotlin => &KOTLIN,
            Self::Scala => &SCALA,
            Self::CSharp => &CSHARP,
            Self::C => &C,
            Self::Cpp => &CPP,
            Self::Php => &PHP,
            Self::Ruby => &RUBY,
            Self::Swift => &SWIFT,
        }
    }

    /// The units of the tree under `root`, in the order the walk meets them.
    fn units<'t>(self, root: Node<'t>, source: &[u8]) -> Vec<Node<'t>> {
        let mut found = Vec::new();
        walk(root, |node, ancestors| {
            let kind = node.kind();
            let inside = |kinds: &[&str]| ancestors.iter().any(|a| kinds.contains(&a.kind()));
            let has_body = || node.child_by_field_name("body").is_some();
            let unit = match self {
                Self::Python => kind == "function_definition" && !python_nested(ancestors),
                Self::TypeScript | Self::Tsx => {
                    if let Some(unit) = script_unit(node, ancestors) {
                        found.push(unit);
                    }
                    false
                }
                Self::Java => {
                    if kind == "method_declaration" {
                        let local = inside(&[
                            "method_declaration",
                            "constructor_declaration",
                            "lambda_expression",
                        ]);
                        if has_child(node, "block") && !local {
                            found.push(node);
                        }
                        // Methods of the classes declared in a method are part
                        // of it.
                        return false;
                    }
                    false
                }
                Self::Go => {
                    matches!(kind, "function_declaration" | "method_declaration")
                        && has_child(node, "block")
                }
                Self::Rust => {
                    kind == "function_item"
                        && has_child(node, "block")
                        && ancestors
                            .last()
                            .is_none_or(|parent| parent.kind() != "block")
                        && !rust_test_module(ancestors, source)
                }
                Self::Kotlin => {
                    kind == "function_declaration"
                        && has_child(node, "function_body")
                        && !inside(&[
                            "function_declaration",
                            "anonymous_function",
                            "lambda_literal",
                        ])
                }
                Self::Scala => {
                    kind == "function_definition"
                        && has_body()
                        && !inside(&["function_definition", "lambda_expression"])
                }
                Self::CSharp => {
                    matches!(
                        kind,
                        "method_declaration"
                            | "operator_declaration"
                            | "conversion_operator_declaration"
                    ) && has_body()
                        && !inside(&[
                            "method_declaration",
                            "constructor_declaration",
                            "destructor_declaration",
                            "operator_declaration",
                            "conversion_operator_declaration",
                            "accessor_declaration",
                            "local_function_statement",
                            "lambda_expression",
                            "anonymous_method_expression",
                        ])
                }
                Self::C | Self::Cpp => {
                    kind == "function_definition"
                        && has_body()
                        && !inside(&["function_definition", "lambda_expression"])
                }
                Self::Php => {
                    matches!(kind, "function_definition" | "method_declaration")
                        && has_body()
                        && !inside(&[
                            "function_definition",
                            "method_declaration",
                            "anonymous_function",
                            "arrow_function",
                        ])
                }
                Self::Ruby => {
                    matches!(kind, "method" | "singleton_method")
                        && !inside(&["method", "singleton_method"])
                }
                Self::Swift => {
                    kind == "function_declaration"
                        && has_body()
                        && !inside(&[
                            "function_declaration",
                            "init_declaration",
                            "deinit_declaration",
                            "lambda_literal",
                        ])
                }
            };
            if unit {
                found.push(node);
            }
            true
        });
        found
    }

    /// The name of a unit, or a stand-in.
    fn name_of(self, unit: Node<'_>, source: &[u8]) -> String {
        let named = unit.child_by_field_name("name").or_else(|| match self {
            Self::C | Self::Cpp => unit
                .child_by_field_name("declarator")
                .and_then(declared_name),
            _ => None,
        });
        named.map_or_else(
            || "<anonymous>".to_string(),
            |name| text(name, source).to_string(),
        )
    }
}

/// The name a C or C++ declarator wraps: `f` in `*f(void)` or `Cart::f()`.
fn declared_name(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        match node.kind() {
            "identifier"
            | "field_identifier"
            | "type_identifier"
            | "destructor_name"
            | "operator_name"
            | "qualified_identifier" => return Some(node),
            "template_function" | "template_method" => {
                node = node.child_by_field_name("name")?;
            }
            _ => {
                node = node
                    .child_by_field_name("declarator")
                    .or_else(|| named_children(node).into_iter().last())?;
            }
        }
    }
}

/// What a grammar calls the nodes the normalizer reads, besides the names
/// most grammars share (the `BASE_` lists).
struct Tables {
    identifiers: &'static [&'static str],
    literals: &'static [&'static str],
    calls: &'static [&'static str],
    attributes: &'static [&'static str],
    /// Paths whose names all stay when a call invokes them.
    paths: &'static [&'static str],
    arguments: &'static [&'static str],
    /// Named nodes that are operators, read by their text: Scala's.
    operators: &'static [&'static str],
    /// Keywords that are literals: Swift's `nil`.
    keyword_literals: &'static [&'static str],
    /// Whether a call keeps what follows its argument list, such as a
    /// trailing lambda.
    trailing: bool,
}

const NONE: &[&str] = &[];

/// Python, JavaScript and TypeScript, Java, Go and Rust: the shared names.
const SHARED: Tables = Tables {
    identifiers: NONE,
    literals: NONE,
    calls: NONE,
    attributes: NONE,
    paths: NONE,
    arguments: NONE,
    operators: NONE,
    keyword_literals: NONE,
    trailing: false,
};

const KOTLIN: Tables = Tables {
    literals: &[
        "number_literal",
        "float_literal",
        "string_literal",
        "multiline_string_literal",
        "unsigned_literal",
        "long_literal",
    ],
    attributes: &["navigation_expression"],
    arguments: &["value_arguments", "annotated_lambda"],
    trailing: true,
    ..SHARED
};

const SCALA: Tables = Tables {
    literals: &["floating_point_literal", "symbol_literal"],
    calls: &["instance_expression"],
    paths: &[
        "stable_identifier",
        "stable_type_identifier",
        "generic_function",
    ],
    operators: &["operator_identifier"],
    trailing: true,
    ..SHARED
};

const CSHARP: Tables = Tables {
    literals: &["real_literal", "verbatim_string_literal"],
    calls: &["invocation_expression"],
    attributes: &["member_access_expression"],
    paths: &["generic_name", "qualified_name"],
    trailing: true,
    ..SHARED
};

const C: Tables = Tables {
    literals: &["number_literal"],
    trailing: true,
    ..SHARED
};

const CPP: Tables = Tables {
    identifiers: &["namespace_identifier"],
    literals: &["number_literal", "user_defined_literal"],
    paths: &[
        "qualified_identifier",
        "template_function",
        "template_method",
        "template_type",
    ],
    trailing: true,
    ..SHARED
};

const PHP: Tables = Tables {
    identifiers: &["name", "variable_name"],
    literals: &["boolean", "encapsed_string", "heredoc", "nowdoc"],
    calls: &[
        "function_call_expression",
        "member_call_expression",
        "nullsafe_member_call_expression",
        "scoped_call_expression",
    ],
    attributes: &[
        "member_access_expression",
        "nullsafe_member_access_expression",
        "scoped_property_access_expression",
        "class_constant_access_expression",
    ],
    paths: &["qualified_name"],
    trailing: true,
    ..SHARED
};

const RUBY: Tables = Tables {
    identifiers: &[
        "constant",
        "instance_variable",
        "class_variable",
        "global_variable",
    ],
    literals: &[
        "simple_symbol",
        "delimited_symbol",
        "hash_key_symbol",
        "character",
        "rational",
        "complex",
    ],
    paths: &["scope_resolution"],
    trailing: true,
    ..SHARED
};

const SWIFT: Tables = Tables {
    identifiers: &["simple_identifier"],
    literals: &[
        "real_literal",
        "hex_literal",
        "oct_literal",
        "bin_literal",
        "line_string_literal",
        "multi_line_string_literal",
    ],
    attributes: &["navigation_expression"],
    arguments: &["call_suffix"],
    keyword_literals: &["nil"],
    trailing: true,
    ..SHARED
};

const BASE_IDENTIFIERS: &[&str] = &[
    "identifier",
    "type_identifier",
    "field_identifier",
    "property_identifier",
    "shorthand_property_identifier",
    "shorthand_property_identifier_pattern",
    "package_identifier",
];

const BASE_LITERALS: &[&str] = &[
    "string",
    "string_literal",
    "interpreted_string_literal",
    "raw_string_literal",
    "char_literal",
    "rune_literal",
    "character_literal",
    "number",
    "integer",
    "float",
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
    "int_literal",
    "float_literal",
    "imaginary_literal",
    "integer_literal",
    "true",
    "false",
    "null",
    "nil",
    "none",
    "undefined",
    "null_literal",
    "boolean_literal",
    "template_string",
    "regex",
    "regex_literal",
];

const BASE_CALLS: &[&str] = &[
    "call",
    "call_expression",
    "method_invocation",
    "object_creation_expression",
    "new_expression",
];

const BASE_ATTRIBUTES: &[&str] = &[
    "attribute",
    "member_expression",
    "selector_expression",
    "field_access",
    "field_expression",
];

const BASE_PATHS: &[&str] = &[
    "scoped_identifier",
    "scoped_type_identifier",
    "generic_type",
];

const BASE_ARGUMENTS: &[&str] = &["argument_list", "arguments"];

const BASE_OPERATORS: &[&str] = &[
    "+", "-", "*", "/", "%", "**", "==", "!=", "<", ">", "<=", ">=", "===", "!==", "&&", "||", "&",
    "|", "^", "<<", ">>", ">>>", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=",
    ">>>=", "!", "~", "++", "--", "and", "or", "not", "??", "?.", "=", ":=", "=>", "?",
];

const COMMENTS: &[&str] = &[
    "comment",
    "line_comment",
    "block_comment",
    "documentation_comment",
    "doc_comment",
];

/// Children of a string that make it code rather than a literal: the
/// replacement fields of a Python f-string, a Kotlin, C# or Ruby template,
/// a Swift interpolation.
const INTERPOLATIONS: &[&str] = &["interpolation", "interpolated_expression"];

impl Tables {
    fn identifier(&self, kind: &str) -> bool {
        BASE_IDENTIFIERS.contains(&kind) || self.identifiers.contains(&kind)
    }

    fn call(&self, kind: &str) -> bool {
        BASE_CALLS.contains(&kind) || self.calls.contains(&kind)
    }

    fn attribute(&self, kind: &str) -> bool {
        BASE_ATTRIBUTES.contains(&kind) || self.attributes.contains(&kind)
    }

    fn path(&self, kind: &str) -> bool {
        BASE_PATHS.contains(&kind) || self.paths.contains(&kind)
    }

    fn arguments(&self, kind: &str) -> bool {
        BASE_ARGUMENTS.contains(&kind) || self.arguments.contains(&kind)
    }

    fn literal(&self, node: Node<'_>) -> bool {
        let kind = node.kind();
        if !(BASE_LITERALS.contains(&kind) || self.literals.contains(&kind)) {
            return false;
        }
        let children = named_children(node);
        if children
            .iter()
            .any(|child| INTERPOLATIONS.contains(&child.kind()))
        {
            return false;
        }
        // A PHP string in double quotes with a variable in it.
        kind != "encapsed_string"
            || children
                .iter()
                .all(|child| matches!(child.kind(), "string_content" | "escape_sequence"))
    }
}

/// A unit of a TypeScript or JavaScript tree at `node`: a function or a
/// method with a body that no function holds, or the variable or field a
/// function is assigned to outside every function. Callbacks are part of
/// the function that passes them.
fn script_unit<'t>(node: Node<'t>, ancestors: &[Node<'t>]) -> Option<Node<'t>> {
    let in_function = |nodes: &[Node<'t>]| nodes.iter().any(|a| is_script_function(a.kind()));
    match node.kind() {
        "function_declaration" | "method_definition" => {
            (has_child(node, "statement_block") && !in_function(ancestors)).then_some(node)
        }
        "arrow_function" | "function_expression" => {
            let (parent, above) = ancestors.split_last()?;
            let definition = matches!(
                parent.kind(),
                "variable_declarator" | "public_field_definition" | "property_definition"
            );
            (definition && !in_function(above)).then_some(*parent)
        }
        _ => None,
    }
}

fn is_script_function(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration" | "method_definition" | "arrow_function" | "function_expression"
    )
}

/// Whether a Python function lies in another function: a class between
/// them makes it a method, which is a unit of its own.
fn python_nested(ancestors: &[Node<'_>]) -> bool {
    for ancestor in ancestors.iter().rev() {
        match ancestor.kind() {
            "class_definition" => return false,
            "function_definition" => return true,
            _ => {}
        }
    }
    false
}

/// Whether a Rust item lies in a `mod tests`.
fn rust_test_module(ancestors: &[Node<'_>], source: &[u8]) -> bool {
    ancestors.iter().any(|ancestor| {
        ancestor.kind() == "mod_item"
            && children(*ancestor)
                .into_iter()
                .find(|child| child.kind() == "identifier")
                .is_some_and(|name| text(name, source) == "tests")
    })
}

fn has_child(node: Node<'_>, kind: &str) -> bool {
    children(node).iter().any(|child| child.kind() == kind)
}

fn children<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    node.children(&mut node.walk()).collect()
}

fn named_children<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    node.named_children(&mut node.walk()).collect()
}

fn text<'s>(node: Node<'_>, source: &'s [u8]) -> &'s str {
    node.utf8_text(source).unwrap_or_default()
}

/// Every node under `root`, root first, in source order; `visit` gets each
/// with the nodes above it, outermost first, and says whether to go into
/// it.
fn walk<'t>(root: Node<'t>, mut visit: impl FnMut(Node<'t>, &[Node<'t>]) -> bool) {
    let mut stack = vec![(root, 0usize)];
    let mut path: Vec<Node<'t>> = Vec::new();
    while let Some((node, depth)) = stack.pop() {
        path.truncate(depth);
        if !visit(node, &path) {
            continue;
        }
        path.push(node);
        for child in children(node).into_iter().rev() {
            stack.push((child, depth + 1));
        }
    }
}

/// A unit found in a source: where it is and its fingerprints.
#[derive(Debug, Clone)]
pub(crate) struct RawForm {
    pub name: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub nodes: u32,
    pub fingerprints: Vec<u64>,
}

/// The units of `source` in `grammar`, without the code in the sorted,
/// disjoint byte ranges `ignored`: a unit that lies inside one is none, and
/// a node that does adds nothing to its unit.
pub(crate) fn forms(source: &str, grammar: Grammar, ignored: &[[usize; 2]]) -> Vec<RawForm> {
    let mut parser = Parser::new();
    if parser
        .set_language(&Language::new(grammar.language()))
        .is_err()
    {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    for unit in grammar.units(tree.root_node(), bytes) {
        if inside(ignored, unit.start_byte(), unit.end_byte()) {
            continue;
        }
        let mut normalizer = Normalizer {
            source: bytes,
            ignored,
            grammar,
            tables: grammar.tables(),
            prints: Prints::default(),
            cache: FxHashMap::default(),
        };
        let root = normalizer.normalize(unit);
        let Value::List { nodes, .. } = root else {
            continue;
        };
        out.push(RawForm {
            name: grammar.name_of(unit, bytes),
            start_byte: unit.start_byte(),
            end_byte: unit.end_byte(),
            nodes,
            fingerprints: normalizer.prints.finish(root),
        });
    }
    out
}

/// Whether `start..end` lies inside one of the sorted, disjoint `ranges`.
pub(crate) fn inside(ranges: &[[usize; 2]], start: usize, end: usize) -> bool {
    let at = ranges.partition_point(|range| range[0] <= start);
    at > 0 && end <= ranges[at - 1][1]
}

/// How a node is read: as code, as the callee of a call, or as a path
/// whose names all stay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Mode {
    Code,
    Callee,
    Path,
}

/// A node to read, how, and whether it is the head of what holds it: a
/// name at the head stays.
type Job<'t> = (Node<'t>, Mode, bool);

/// The parts of a call: what it is called on, what it calls, the type
/// arguments, the argument list and what follows it.
struct CallParts<'t> {
    receivers: Vec<Node<'t>>,
    callee: Option<Node<'t>>,
    type_arguments: Vec<Node<'t>>,
    arguments: Option<Node<'t>>,
    trailing: Vec<Node<'t>>,
}

struct Normalizer<'s> {
    source: &'s [u8],
    ignored: &'s [[usize; 2]],
    grammar: Grammar,
    tables: &'static Tables,
    prints: Prints,
    cache: FxHashMap<(usize, Mode, bool), Value>,
}

impl Normalizer<'_> {
    /// The normalized tree of `root`. A walk with its own stack, children
    /// before their parent: a thousand nested calls must not overflow the
    /// thread's stack.
    fn normalize(&mut self, root: Node<'_>) -> Value {
        let mut stack = vec![(root, Mode::Code, false, false)];
        while let Some((node, mode, head, expanded)) = stack.pop() {
            let key = (node.id(), mode, head);
            if expanded {
                let value = self.assemble(node, mode, head);
                self.cache.insert(key, value);
                continue;
            }
            if self.cache.contains_key(&key) {
                continue;
            }
            if let Some(leaf) = self.leaf(node, mode, head) {
                self.cache.insert(key, leaf);
                continue;
            }
            stack.push((node, mode, head, true));
            for (child, mode, head) in self.jobs(node, mode, head).into_iter().rev() {
                stack.push((child, mode, head, false));
            }
        }
        self.value(root, Mode::Code, false)
    }

    fn value(&self, node: Node<'_>, mode: Mode, head: bool) -> Value {
        self.cache
            .get(&(node.id(), mode, head))
            .copied()
            .unwrap_or(Value::None)
    }

    /// The value of a node that holds no node to read first, `None` for one
    /// whose children come first.
    fn leaf(&mut self, node: Node<'_>, mode: Mode, head: bool) -> Option<Value> {
        if inside(self.ignored, node.start_byte(), node.end_byte()) {
            return Some(Value::None);
        }
        let kind = node.kind();
        let tables = self.tables;
        match mode {
            Mode::Callee => {
                return tables.identifier(kind).then(|| {
                    let name = text(node, self.source);
                    self.prints.symbol(name)
                });
            }
            Mode::Path => return None,
            Mode::Code => {}
        }
        if COMMENTS.contains(&kind) {
            return Some(Value::None);
        }
        if BASE_OPERATORS.contains(&kind) {
            return Some(self.prints.symbol(kind));
        }
        if tables.operators.contains(&kind) {
            let operator = text(node, self.source);
            return Some(self.prints.symbol(operator));
        }
        if !node.is_named() {
            return Some(match tables.keyword_literals.contains(&kind) {
                true => Value::Atom(keyword("literal")),
                false => Value::None,
            });
        }
        if tables.identifier(kind) {
            return Some(match head {
                true => {
                    let name = text(node, self.source);
                    self.prints.symbol(name)
                }
                false => Value::Atom(keyword("symbol")),
            });
        }
        if tables.literal(node) {
            return Some(Value::Atom(keyword("literal")));
        }
        None
    }

    fn jobs<'t>(&self, node: Node<'t>, mode: Mode, head: bool) -> Vec<Job<'t>> {
        match mode {
            Mode::Callee => self.callee_jobs(node),
            Mode::Path => self.path_jobs(node),
            Mode::Code => {
                if let Some(inner) = only_parenthesized(node) {
                    return vec![(inner, Mode::Code, head)];
                }
                let kind = node.kind();
                if self.tables.call(kind) {
                    let parts = self.call_parts(node);
                    let mut jobs: Vec<Job<'t>> = parts
                        .receivers
                        .iter()
                        .map(|r| (*r, Mode::Code, false))
                        .collect();
                    jobs.extend(parts.callee.map(|c| (c, Mode::Callee, false)));
                    jobs.extend(parts.type_arguments.iter().map(|t| (*t, Mode::Code, false)));
                    jobs.extend(parts.arguments.map(|a| (a, Mode::Code, false)));
                    jobs.extend(parts.trailing.iter().map(|t| (*t, Mode::Code, false)));
                    jobs
                } else if self.tables.attribute(kind) {
                    self.attribute_jobs(node, head)
                } else {
                    children(node)
                        .into_iter()
                        .map(|child| (child, Mode::Code, false))
                        .collect()
                }
            }
        }
    }

    fn assemble(&mut self, node: Node<'_>, mode: Mode, head: bool) -> Value {
        match mode {
            Mode::Callee => {
                let kind = node.kind();
                if self.tables.attribute(kind) {
                    self.attribute(node, true)
                } else if self.tables.path(kind) {
                    self.path(node)
                } else {
                    self.value(node, Mode::Code, true)
                }
            }
            Mode::Path => self.path(node),
            Mode::Code => {
                if let Some(inner) = only_parenthesized(node) {
                    return self.value(inner, Mode::Code, head);
                }
                let kind = node.kind();
                if self.tables.call(kind) {
                    self.call(node)
                } else if self.tables.attribute(kind) {
                    self.attribute(node, head)
                } else {
                    let mut parts = vec![Value::Atom(keyword(kind))];
                    for child in children(node) {
                        parts.push(self.value(child, Mode::Code, false));
                    }
                    self.prints.list(&parts)
                }
            }
        }
    }

    /// What a call is made of. A Ruby call names its parts by field: its
    /// receiver, its method, its arguments and its block. Elsewhere the named
    /// children before the argument list are the receivers and, last, the
    /// callee.
    fn call_parts<'t>(&self, node: Node<'t>) -> CallParts<'t> {
        if self.grammar == Grammar::Ruby && node.kind() == "call" {
            let field = |name: &str| node.child_by_field_name(name);
            return CallParts {
                receivers: field("receiver").into_iter().collect(),
                callee: field("method"),
                type_arguments: Vec::new(),
                arguments: field("arguments"),
                trailing: field("block").into_iter().collect(),
            };
        }
        let mut before = Vec::new();
        let mut arguments = None;
        let mut trailing = Vec::new();
        for child in named_children(node) {
            if arguments.is_some() {
                if self.tables.trailing {
                    trailing.push(child);
                }
            } else if self.tables.arguments(child.kind()) {
                arguments = Some(child);
            } else {
                before.push(child);
            }
        }
        let (mut rest, type_arguments): (Vec<_>, Vec<_>) = before
            .into_iter()
            .partition(|n| !matches!(n.kind(), "type_arguments" | "type_parameters"));
        let callee = rest.pop();
        CallParts {
            receivers: rest,
            callee,
            type_arguments,
            arguments,
            trailing,
        }
    }

    /// `[:call receivers… callee type-arguments… arguments trailing…]`.
    fn call(&mut self, node: Node<'_>) -> Value {
        let parts = self.call_parts(node);
        let mut values = vec![Value::Atom(keyword(node.kind()))];
        for receiver in &parts.receivers {
            values.push(self.value(*receiver, Mode::Code, false));
        }
        if let Some(callee) = parts.callee {
            values.push(self.value(callee, Mode::Callee, false));
        }
        for argument in &parts.type_arguments {
            values.push(self.value(*argument, Mode::Code, false));
        }
        if let Some(arguments) = parts.arguments {
            values.push(self.value(arguments, Mode::Code, false));
        }
        for node in &parts.trailing {
            values.push(self.value(*node, Mode::Code, false));
        }
        self.prints.list(&values)
    }

    /// The objects of an attribute and its name: the name a Swift
    /// navigation suffix holds.
    fn attribute_parts<'t>(&self, node: Node<'t>) -> Option<(Vec<Node<'t>>, Node<'t>)> {
        let mut named = named_children(node);
        let name = named.pop()?;
        let name = match name.kind() {
            "navigation_suffix" => named_children(name).into_iter().last().unwrap_or(name),
            _ => name,
        };
        Some((named, name))
    }

    /// `[:attribute objects… name]`: the name stays when the attribute is
    /// what a call invokes.
    fn attribute(&mut self, node: Node<'_>, head: bool) -> Value {
        let mut parts = vec![Value::Atom(keyword(node.kind()))];
        if let Some((objects, name)) = self.attribute_parts(node) {
            for object in objects {
                parts.push(self.value(object, Mode::Code, false));
            }
            let value = match (self.tables.identifier(name.kind()), head) {
                (true, true) => {
                    let text = text(name, self.source);
                    self.prints.symbol(text)
                }
                (true, false) => Value::Atom(keyword("symbol")),
                (false, true) => self.value(name, Mode::Callee, false),
                (false, false) => self.value(name, Mode::Code, false),
            };
            parts.push(value);
        }
        self.prints.list(&parts)
    }

    /// A path a call invokes keeps every name in it; type arguments stay
    /// structure.
    fn path(&mut self, node: Node<'_>) -> Value {
        let mut parts = vec![Value::Atom(keyword(node.kind()))];
        for child in named_children(node) {
            let kind = child.kind();
            let value = if self.tables.identifier(kind) {
                let text = text(child, self.source);
                self.prints.symbol(text)
            } else if self.tables.path(kind) {
                self.value(child, Mode::Path, false)
            } else {
                self.value(child, Mode::Code, false)
            };
            parts.push(value);
        }
        self.prints.list(&parts)
    }

    fn callee_jobs<'t>(&self, node: Node<'t>) -> Vec<Job<'t>> {
        let kind = node.kind();
        if self.tables.attribute(kind) {
            self.attribute_jobs(node, true)
        } else if self.tables.path(kind) {
            self.path_jobs(node)
        } else {
            vec![(node, Mode::Code, true)]
        }
    }

    fn attribute_jobs<'t>(&self, node: Node<'t>, head: bool) -> Vec<Job<'t>> {
        let Some((objects, name)) = self.attribute_parts(node) else {
            return Vec::new();
        };
        let mut jobs: Vec<Job<'t>> = objects.iter().map(|o| (*o, Mode::Code, false)).collect();
        if !self.tables.identifier(name.kind()) {
            let mode = match head {
                true => Mode::Callee,
                false => Mode::Code,
            };
            jobs.push((name, mode, false));
        }
        jobs
    }

    fn path_jobs<'t>(&self, node: Node<'t>) -> Vec<Job<'t>> {
        named_children(node)
            .into_iter()
            .filter(|child| !self.tables.identifier(child.kind()))
            .map(|child| match self.tables.path(child.kind()) {
                true => (child, Mode::Path, false),
                false => (child, Mode::Code, false),
            })
            .collect()
    }
}

/// The expression in parentheses that hold nothing else.
fn only_parenthesized(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() != "parenthesized_expression" {
        return None;
    }
    let named = named_children(node);
    match named.as_slice() {
        [inner] => Some(*inner),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(a: &RawForm, b: &RawForm) -> f64 {
        crate::join::jaccard(&a.fingerprints, &b.fingerprints)
    }

    fn only(source: &str, format: &str) -> RawForm {
        let mut forms = forms(source, Grammar::for_format(format).unwrap(), &[]);
        assert_eq!(forms.len(), 1, "{format}: {forms:?}");
        forms.remove(0)
    }

    fn names(source: &str, format: &str) -> Vec<String> {
        forms(source, Grammar::for_format(format).unwrap(), &[])
            .into_iter()
            .map(|f| f.name)
            .collect()
    }

    #[test]
    fn renamed_locals_and_other_literals_match_while_another_call_does_not() {
        let a = only(
            "def alpha(xs):\n    ys = filter(xs, 1)\n    return map(ys, inc)\n",
            "python",
        );
        let b = only(
            "def beta(items):\n    kept = filter(items, 2)\n    return map(kept, dec)\n",
            "python",
        );
        let c = only(
            "def gamma(items):\n    kept = sorted(items, 2)\n    return map(kept, dec)\n",
            "python",
        );
        assert_eq!(score(&a, &b), 1.0);
        assert!(score(&a, &c) < 1.0);
        assert_eq!(a.name, "alpha");
    }

    #[test]
    fn operators_count_and_parentheses_around_one_expression_do_not() {
        let plus = only("function f(a, b) { return a + b * 2; }", "typescript");
        let minus = only("function f(a, b) { return a - b * 2; }", "typescript");
        let grouped = only("function f(a, b) { return (a + (b * 2)); }", "typescript");
        assert!(score(&plus, &minus) < 1.0);
        assert_eq!(score(&plus, &grouped), 1.0);
    }

    #[test]
    fn a_method_keeps_its_name_and_loses_its_receiver() {
        let a = only("func A(s *Store) { s.Save(1) }", "go");
        let b = only("func B(t *Store) { t.Save(2) }", "go");
        let c = only("func C(t *Store) { t.Load(2) }", "go");
        assert_eq!(score(&a, &b), 1.0);
        assert!(score(&a, &c) < 1.0);
    }

    #[test]
    fn units_follow_each_language() {
        let python = "def outer():\n    def inner():\n        pass\n    class Local:\n        def method(self):\n            pass\n";
        assert_eq!(names(python, "python"), ["outer", "method"]);
        let script = "export const load = async (id) => { return get(id); };\nfunction top() { return [1].map(x => x); }\nclass A { handle = () => { run(); }; save() { this.x(); } }\n";
        assert_eq!(
            names(script, "javascript"),
            ["load", "top", "handle", "save"]
        );
        let rust = "fn a() { fn b() {} }\nmod tests { fn c() {} }\nimpl S { fn d(&self) {} }\ntrait T { fn e(); }\n";
        assert_eq!(names(rust, "rust"), ["a", "d"]);
        let java = "class A { void m() { new Runnable() { public void run() {} }; } abstract void n(); A() {} }";
        assert_eq!(names(java, "java"), ["m"]);
        let kotlin = "class A { fun m(): Int { fun local() = 1; return local() }\n  abstract fun n(): Int }\nfun top() { listOf(1).map { it } }\n";
        assert_eq!(names(kotlin, "kotlin"), ["m", "top"]);
        let scala = "object A { def m(x: Int): Int = { def local(y: Int) = y; local(x) }\n  def n(): Unit }\n";
        assert_eq!(names(scala, "scala"), ["m"]);
        let csharp = "class A { int M() { int Local() => 1; return Local(); }\n  int N() => 2;\n  abstract int O();\n  A() { } }";
        assert_eq!(names(csharp, "csharp"), ["M", "N"]);
        let c = "static int total(int *xs, int n) { return n; }\nint decl(void);\n";
        assert_eq!(names(c, "c"), ["total"]);
        let cpp = "int Cart::total() const { auto f = [](int x) { return x; }; return f(1); }\nstruct S { void m() {} S() = default; };\n";
        assert_eq!(names(cpp, "cpp"), ["Cart::total", "m"]);
        let php = "<?php\nfunction top($a) { $f = function () { return 1; }; return $f(); }\nclass A { public function m() { return 1; } abstract function n(); }\n";
        assert_eq!(names(php, "php"), ["top", "m"]);
        let ruby =
            "class A\n  def m(x)\n    def inner; end\n    x\n  end\n  def self.n; end\nend\n";
        assert_eq!(names(ruby, "ruby"), ["m", "n"]);
        let swift = "class A { func m() -> Int { func local() -> Int { 1 }; return local() }\n  init() { } }\nfunc top() { }\n";
        assert_eq!(names(swift, "swift"), ["m", "top"]);
    }

    /// A renamed copy with other literals scores 1, and another method
    /// called scores less, in every language.
    #[test]
    fn every_language_reads_names_literals_and_calls_alike() {
        let cases: &[(&str, &str, &str, &str)] = &[
            (
                "kotlin",
                "fun total(items: List<Item>, rate: Rate): Int {\n  var sum = 0\n  for (item in items) { if (item.active) { sum += item.price * 2 } }\n  return rate.apply(sum, \"x\")\n}\n",
                "fun amount(rows: List<Row>, tax: Tax): Int {\n  var acc = 0\n  for (row in rows) { if (row.active) { acc += row.price * 3 } }\n  return tax.apply(acc, \"y\")\n}\n",
                "fun amount(rows: List<Row>, tax: Tax): Int {\n  var acc = 0\n  for (row in rows) { if (row.active) { acc += row.price * 3 } }\n  return tax.round(acc, \"y\")\n}\n",
            ),
            (
                "scala",
                "object A { def total(items: List[Item], rate: Rate): Int = { var sum = 0; for (item <- items) { if (item.active) { sum += item.price * 2 } }; rate.apply(sum, \"x\") } }\n",
                "object B { def amount(rows: List[Row], tax: Tax): Int = { var acc = 0; for (row <- rows) { if (row.active) { acc += row.price * 3 } }; tax.apply(acc, \"y\") } }\n",
                "object B { def amount(rows: List[Row], tax: Tax): Int = { var acc = 0; for (row <- rows) { if (row.active) { acc -= row.price * 3 } }; tax.apply(acc, \"y\") } }\n",
            ),
            (
                "csharp",
                "class A { int Total(List<Item> items, Rate rate) { var sum = 0; foreach (var item in items) { if (item.Active) { sum += item.Price * 2; } } return rate.Apply(sum, \"x\"); } }\n",
                "class B { int Amount(List<Row> rows, Tax tax) { var acc = 0; foreach (var row in rows) { if (row.Active) { acc += row.Price * 3; } } return tax.Apply(acc, \"y\"); } }\n",
                "class B { int Amount(List<Row> rows, Tax tax) { var acc = 0; foreach (var row in rows) { if (row.Active) { acc += row.Price * 3; } } return tax.Round(acc, \"y\"); } }\n",
            ),
            (
                "c",
                "int total(struct item *items, int n, struct rate *rate) { int sum = 0; for (int i = 0; i < n; i++) { if (items[i].active) { sum += items[i].price * 2; } } return apply(rate, sum, \"x\"); }\n",
                "int amount(struct row *rows, int m, struct tax *tax) { int acc = 0; for (int j = 0; j < m; j++) { if (rows[j].active) { acc += rows[j].price * 3; } } return apply(tax, acc, \"y\"); }\n",
                "int amount(struct row *rows, int m, struct tax *tax) { int acc = 0; for (int j = 0; j < m; j++) { if (rows[j].active) { acc += rows[j].price * 3; } } return round(tax, acc, \"y\"); }\n",
            ),
            (
                "cpp",
                "int total(const std::vector<Item>& items, Rate& rate) { int sum = 0; for (const auto& item : items) { if (item.active) { sum += item.price * 2; } } return std::max(rate.apply(sum), 0); }\n",
                "int amount(const std::vector<Row>& rows, Tax& tax) { int acc = 0; for (const auto& row : rows) { if (row.active) { acc += row.price * 3; } } return std::max(tax.apply(acc), 1); }\n",
                "int amount(const std::vector<Row>& rows, Tax& tax) { int acc = 0; for (const auto& row : rows) { if (row.active) { acc += row.price * 3; } } return std::min(tax.apply(acc), 1); }\n",
            ),
            (
                "php",
                "<?php\nfunction total(array $items, Rate $rate): int { $sum = 0; foreach ($items as $item) { if ($item->active) { $sum += $item->price * 2; } } return $rate->apply($sum, 'x'); }\n",
                "<?php\nfunction amount(array $rows, Tax $tax): int { $acc = 0; foreach ($rows as $row) { if ($row->active) { $acc += $row->price * 3; } } return $tax->apply($acc, \"y\"); }\n",
                "<?php\nfunction amount(array $rows, Tax $tax): int { $acc = 0; foreach ($rows as $row) { if ($row->active) { $acc += $row->price * 3; } } return $tax->round($acc, \"y\"); }\n",
            ),
            (
                "ruby",
                "def total(items, rate)\n  sum = 0\n  items.each do |item|\n    sum += item.price * 2 if item.active\n  end\n  rate.apply(sum, 'x')\nend\n",
                "def amount(rows, tax)\n  acc = 0\n  rows.each do |row|\n    acc += row.price * 3 if row.active\n  end\n  tax.apply(acc, :y)\nend\n",
                "def amount(rows, tax)\n  acc = 0\n  rows.each do |row|\n    acc += row.price * 3 if row.active\n  end\n  tax.round(acc, :y)\nend\n",
            ),
            (
                "swift",
                "func total(items: [Item], rate: Rate) -> Int { var sum = 0; for item in items { if item.active { sum += item.price * 2 } }; return rate.apply(sum, \"x\") }\n",
                "func amount(rows: [Row], tax: Tax) -> Int { var acc = 0; for row in rows { if row.active { acc += row.price * 3 } }; return tax.apply(acc, nil) }\n",
                "func amount(rows: [Row], tax: Tax) -> Int { var acc = 0; for row in rows { if row.active { acc += row.price * 3 } }; return tax.round(acc, nil) }\n",
            ),
        ];
        for (format, a, b, c) in cases {
            let (a, b, c) = (only(a, format), only(b, format), only(c, format));
            assert_eq!(score(&a, &b), 1.0, "{format}: a renamed copy");
            assert!(score(&b, &c) < 1.0, "{format}: another call or operator");
        }
    }

    #[test]
    fn ignored_code_adds_nothing_and_a_unit_inside_it_is_none() {
        let source = "def a(x):\n    log(x)\n    return x\n\ndef b(x):\n    return x\n";
        let log = source.find("log").unwrap();
        let ignored = [[log, log + "log(x)".len()]];
        let all = forms(source, Grammar::Python, &ignored);
        assert_eq!(score(&all[0], &all[1]), 1.0);
        let start = source.find("def b").unwrap();
        let none = forms(source, Grammar::Python, &[[start, source.len()]]);
        assert_eq!(none.len(), 1);
    }
}
