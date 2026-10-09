use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct FormatEntry {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub parent: Option<&'static str>,
}

/// All supported language formats (223 entries).
pub static SUPPORTED_FORMATS: &[FormatEntry] = &[
    FormatEntry {
        name: "abap",
        extensions: &["abap"],
        parent: None,
    },
    FormatEntry {
        name: "actionscript",
        extensions: &["as"],
        parent: None,
    },
    FormatEntry {
        name: "ada",
        extensions: &["ada"],
        parent: None,
    },
    FormatEntry {
        name: "apacheconf",
        extensions: &["apacheconf"],
        parent: None,
    },
    FormatEntry {
        name: "apl",
        extensions: &["apl"],
        parent: None,
    },
    FormatEntry {
        name: "applescript",
        extensions: &["applescript"],
        parent: None,
    },
    FormatEntry {
        name: "arduino",
        extensions: &["ino"],
        parent: None,
    },
    FormatEntry {
        name: "arff",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "asciidoc",
        extensions: &["adoc", "asciidoc"],
        parent: None,
    },
    FormatEntry {
        name: "asm6502",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "aspnet",
        extensions: &["asp", "aspx"],
        parent: None,
    },
    FormatEntry {
        name: "autohotkey",
        extensions: &["ahk", "ah2"],
        parent: None,
    },
    FormatEntry {
        name: "autoit",
        extensions: &["au3"],
        parent: None,
    },
    FormatEntry {
        name: "bash",
        extensions: &["sh", "ksh", "bash", "zsh", "bats"],
        parent: None,
    },
    FormatEntry {
        name: "basic",
        extensions: &["bas"],
        parent: None,
    },
    FormatEntry {
        name: "batch",
        extensions: &["bat", "cmd"],
        parent: None,
    },
    FormatEntry {
        name: "bison",
        extensions: &["y", "yacc", "bison"],
        parent: None,
    },
    FormatEntry {
        name: "brainfuck",
        extensions: &["b", "bf"],
        parent: None,
    },
    FormatEntry {
        name: "bro",
        extensions: &["zeek", "bro"],
        parent: None,
    },
    FormatEntry {
        name: "c",
        extensions: &["c", "z80"],
        parent: None,
    },
    FormatEntry {
        name: "c-header",
        extensions: &["h"],
        parent: None,
    },
    FormatEntry {
        name: "clike",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "clojure",
        extensions: &["cljs", "clj", "cljc", "cljd", "cljx", "edn"],
        parent: None,
    },
    FormatEntry {
        name: "coffeescript",
        extensions: &["coffee"],
        parent: None,
    },
    FormatEntry {
        name: "comments",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "cpp",
        extensions: &["cpp", "c++", "cc", "cxx", "cppm", "ixx"],
        parent: None,
    },
    FormatEntry {
        name: "cpp-header",
        extensions: &["hpp", "h++", "hh", "hxx", "ipp", "tpp", "inl"],
        parent: None,
    },
    FormatEntry {
        name: "crystal",
        extensions: &["cr"],
        parent: None,
    },
    FormatEntry {
        name: "csharp",
        extensions: &["cs", "csx"],
        parent: None,
    },
    FormatEntry {
        name: "csp",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "css-extras",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "css",
        extensions: &["css", "gss"],
        parent: None,
    },
    FormatEntry {
        name: "d",
        extensions: &["d"],
        parent: None,
    },
    FormatEntry {
        name: "dart",
        extensions: &["dart"],
        parent: None,
    },
    FormatEntry {
        name: "diff",
        extensions: &["diff", "patch"],
        parent: None,
    },
    FormatEntry {
        name: "django",
        extensions: &["jinja", "jinja2", "j2"],
        parent: None,
    },
    FormatEntry {
        name: "docker",
        extensions: &["dockerfile", "Dockerfile", "containerfile"],
        parent: None,
    },
    FormatEntry {
        name: "eiffel",
        extensions: &["e"],
        parent: None,
    },
    FormatEntry {
        name: "elixir",
        extensions: &["ex", "exs"],
        parent: None,
    },
    FormatEntry {
        name: "elm",
        extensions: &["elm"],
        parent: None,
    },
    FormatEntry {
        name: "erb",
        extensions: &["erb", "rhtml"],
        parent: None,
    },
    FormatEntry {
        name: "erlang",
        extensions: &["erl", "erlang", "hrl"],
        parent: None,
    },
    FormatEntry {
        name: "flow",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "fortran",
        extensions: &["f", "for", "f77", "f90"],
        parent: None,
    },
    FormatEntry {
        name: "fsharp",
        extensions: &["fs", "fsi", "fsx"],
        parent: None,
    },
    FormatEntry {
        name: "gdscript",
        extensions: &["gd"],
        parent: None,
    },
    FormatEntry {
        name: "gedcom",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "gherkin",
        extensions: &["feature"],
        parent: None,
    },
    FormatEntry {
        name: "git",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "glsl",
        extensions: &[
            "glsl", "vert", "frag", "geom", "tesc", "tese", "comp", "rgen", "rint", "rahit",
            "rchit", "rmiss", "rcall",
        ],
        parent: None,
    },
    FormatEntry {
        name: "go",
        extensions: &["go"],
        parent: None,
    },
    FormatEntry {
        name: "graphql",
        extensions: &["graphql"],
        parent: None,
    },
    FormatEntry {
        name: "groovy",
        extensions: &["groovy", "gradle", "gvy"],
        parent: None,
    },
    FormatEntry {
        name: "haml",
        extensions: &["haml"],
        parent: None,
    },
    FormatEntry {
        name: "handlebars",
        extensions: &["hb", "hbs", "handlebars"],
        parent: None,
    },
    FormatEntry {
        name: "haskell",
        extensions: &["hs", "lhs"],
        parent: None,
    },
    FormatEntry {
        name: "haxe",
        extensions: &["hx", "hxml"],
        parent: None,
    },
    FormatEntry {
        name: "hpkp",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "hsts",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "http",
        extensions: &["http"],
        parent: None,
    },
    FormatEntry {
        name: "ichigojam",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "icon",
        extensions: &["icn"],
        parent: None,
    },
    FormatEntry {
        name: "inform7",
        extensions: &["ni", "i7x"],
        parent: None,
    },
    FormatEntry {
        name: "ini",
        extensions: &["ini"],
        parent: None,
    },
    FormatEntry {
        name: "io",
        extensions: &["io"],
        parent: None,
    },
    FormatEntry {
        name: "j",
        extensions: &["ijs"],
        parent: None,
    },
    FormatEntry {
        name: "java",
        extensions: &["java"],
        parent: None,
    },
    FormatEntry {
        name: "javascript",
        extensions: &["js", "es", "es6", "mjs", "cjs"],
        parent: None,
    },
    FormatEntry {
        name: "jolie",
        extensions: &["ol", "iol"],
        parent: None,
    },
    FormatEntry {
        name: "json",
        extensions: &["json", "map", "jsonld", "jsonc", "webmanifest"],
        parent: None,
    },
    FormatEntry {
        name: "jsx",
        extensions: &["jsx"],
        parent: None,
    },
    FormatEntry {
        name: "julia",
        extensions: &["jl"],
        parent: None,
    },
    FormatEntry {
        name: "keymap",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "kotlin",
        extensions: &["kt", "kts"],
        parent: None,
    },
    FormatEntry {
        name: "latex",
        extensions: &["tex"],
        parent: None,
    },
    FormatEntry {
        name: "less",
        extensions: &["less"],
        parent: None,
    },
    FormatEntry {
        name: "liquid",
        extensions: &["liquid"],
        parent: None,
    },
    FormatEntry {
        name: "lisp",
        extensions: &["cl", "lisp", "el"],
        parent: None,
    },
    FormatEntry {
        name: "livescript",
        extensions: &["ls"],
        parent: None,
    },
    FormatEntry {
        name: "lolcode",
        extensions: &["lol"],
        parent: None,
    },
    FormatEntry {
        name: "lua",
        extensions: &["lua"],
        parent: None,
    },
    FormatEntry {
        name: "makefile",
        extensions: &["mk", "mak", "make"],
        parent: None,
    },
    FormatEntry {
        name: "markdown",
        extensions: &["md", "markdown", "mkd"],
        parent: None,
    },
    FormatEntry {
        name: "markup",
        extensions: &[
            "html", "htm", "xml", "xsl", "xslt", "svg", "ejs", "jsp", "xhtml",
        ],
        parent: None,
    },
    FormatEntry {
        name: "matlab",
        extensions: &["matlab"],
        parent: None,
    },
    FormatEntry {
        name: "mel",
        extensions: &["mel"],
        parent: None,
    },
    FormatEntry {
        name: "mizar",
        extensions: &["miz"],
        parent: None,
    },
    FormatEntry {
        name: "monkey",
        extensions: &["monkey", "monkey2"],
        parent: None,
    },
    FormatEntry {
        name: "n4js",
        extensions: &["n4js", "n4jsd"],
        parent: None,
    },
    FormatEntry {
        name: "nasm",
        extensions: &["asm", "nasm", "nas"],
        parent: None,
    },
    FormatEntry {
        name: "nginx",
        extensions: &["nginx", "nginxconf"],
        parent: None,
    },
    FormatEntry {
        name: "nim",
        extensions: &["nim", "nims", "nimble"],
        parent: None,
    },
    FormatEntry {
        name: "nix",
        extensions: &["nix"],
        parent: None,
    },
    FormatEntry {
        name: "nsis",
        extensions: &["nsh", "nsi"],
        parent: None,
    },
    FormatEntry {
        name: "objectivec",
        extensions: &["m", "mm"],
        parent: None,
    },
    FormatEntry {
        name: "ocaml",
        extensions: &["ocaml", "ml", "mli", "mll", "mly"],
        parent: None,
    },
    FormatEntry {
        name: "opencl",
        extensions: &["opencl"],
        parent: None,
    },
    FormatEntry {
        name: "oz",
        extensions: &["oz"],
        parent: None,
    },
    FormatEntry {
        name: "parigp",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "pascal",
        extensions: &["pas", "p"],
        parent: None,
    },
    FormatEntry {
        name: "perl",
        extensions: &["pl", "pm"],
        parent: None,
    },
    FormatEntry {
        name: "php",
        extensions: &["php", "phtml"],
        parent: None,
    },
    FormatEntry {
        name: "plsql",
        extensions: &["plsql"],
        parent: None,
    },
    FormatEntry {
        name: "powershell",
        extensions: &["ps1", "psd1", "psm1"],
        parent: None,
    },
    FormatEntry {
        name: "processing",
        extensions: &["pde"],
        parent: None,
    },
    FormatEntry {
        name: "prolog",
        extensions: &["pro"],
        parent: None,
    },
    FormatEntry {
        name: "properties",
        extensions: &["properties"],
        parent: None,
    },
    FormatEntry {
        name: "protobuf",
        extensions: &["proto"],
        parent: None,
    },
    FormatEntry {
        name: "pug",
        extensions: &["pug", "jade"],
        parent: None,
    },
    FormatEntry {
        name: "puppet",
        extensions: &["pp", "puppet"],
        parent: None,
    },
    FormatEntry {
        name: "pure",
        extensions: &["pure"],
        parent: None,
    },
    FormatEntry {
        name: "python",
        extensions: &["py", "pyi", "pyx", "pxd", "pxi"],
        parent: None,
    },
    FormatEntry {
        name: "q",
        extensions: &["q"],
        parent: None,
    },
    FormatEntry {
        name: "qore",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "r",
        extensions: &["r", "R"],
        parent: None,
    },
    FormatEntry {
        name: "razor",
        extensions: &["cshtml", "razor"],
        parent: None,
    },
    FormatEntry {
        name: "reason",
        extensions: &["re", "rei"],
        parent: None,
    },
    FormatEntry {
        name: "renpy",
        extensions: &["rpy"],
        parent: None,
    },
    FormatEntry {
        name: "rest",
        extensions: &["rst", "rest"],
        parent: None,
    },
    FormatEntry {
        name: "rip",
        extensions: &["rip"],
        parent: None,
    },
    FormatEntry {
        name: "roboconf",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "ruby",
        extensions: &["rb", "rake", "gemspec", "ru", "podspec", "jbuilder", "thor"],
        parent: None,
    },
    FormatEntry {
        name: "rust",
        extensions: &["rs"],
        parent: None,
    },
    FormatEntry {
        name: "sas",
        extensions: &["sas"],
        parent: None,
    },
    FormatEntry {
        name: "sass",
        extensions: &["sass"],
        parent: None,
    },
    FormatEntry {
        name: "scala",
        extensions: &["scala", "sbt"],
        parent: None,
    },
    FormatEntry {
        name: "scheme",
        extensions: &["scm", "ss"],
        parent: None,
    },
    FormatEntry {
        name: "scss",
        extensions: &["scss"],
        parent: None,
    },
    FormatEntry {
        name: "svelte",
        extensions: &["svelte"],
        parent: None,
    },
    FormatEntry {
        name: "smalltalk",
        extensions: &["st"],
        parent: None,
    },
    FormatEntry {
        name: "smarty",
        extensions: &["smarty", "tpl"],
        parent: None,
    },
    FormatEntry {
        name: "soy",
        extensions: &["soy"],
        parent: None,
    },
    FormatEntry {
        name: "sql",
        extensions: &["sql", "cql"],
        parent: None,
    },
    FormatEntry {
        name: "stylus",
        extensions: &["styl", "stylus"],
        parent: None,
    },
    FormatEntry {
        name: "swift",
        extensions: &["swift"],
        parent: None,
    },
    FormatEntry {
        name: "tap",
        extensions: &["tap"],
        parent: None,
    },
    FormatEntry {
        name: "tcl",
        extensions: &["tcl"],
        parent: None,
    },
    FormatEntry {
        name: "textile",
        extensions: &["textile"],
        parent: None,
    },
    FormatEntry {
        name: "tsx",
        extensions: &["tsx"],
        parent: None,
    },
    FormatEntry {
        name: "tt2",
        extensions: &["tt2"],
        parent: None,
    },
    FormatEntry {
        name: "twig",
        extensions: &["twig"],
        parent: None,
    },
    FormatEntry {
        name: "typescript",
        extensions: &["ts", "mts", "cts"],
        parent: None,
    },
    FormatEntry {
        name: "txt",
        extensions: &["txt"],
        parent: None,
    },
    FormatEntry {
        name: "vbnet",
        extensions: &["vb"],
        parent: None,
    },
    FormatEntry {
        name: "velocity",
        extensions: &["vtl"],
        parent: None,
    },
    FormatEntry {
        name: "verilog",
        extensions: &["v"],
        parent: None,
    },
    FormatEntry {
        name: "vhdl",
        extensions: &["vhd", "vhdl"],
        parent: None,
    },
    FormatEntry {
        name: "vim",
        extensions: &["vim"],
        parent: None,
    },
    FormatEntry {
        name: "visual-basic",
        extensions: &["vbs", "vba"],
        parent: None,
    },
    FormatEntry {
        name: "astro",
        extensions: &["astro"],
        parent: None,
    },
    FormatEntry {
        name: "vue",
        extensions: &["vue"],
        parent: None,
    },
    FormatEntry {
        name: "wasm",
        extensions: &["wat", "wast"],
        parent: None,
    },
    FormatEntry {
        name: "wiki",
        extensions: &["wiki", "mediawiki", "wikitext"],
        parent: None,
    },
    FormatEntry {
        name: "xeora",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "xojo",
        extensions: &[
            "xojo_code",
            "xojo_script",
            "xojo_window",
            "xojo_menu",
            "xojo_report",
            "xojo_toolbar",
        ],
        parent: None,
    },
    FormatEntry {
        name: "xquery",
        extensions: &["xy", "xquery"],
        parent: None,
    },
    FormatEntry {
        name: "yaml",
        extensions: &["yaml", "yml"],
        parent: None,
    },
    FormatEntry {
        name: "abnf",
        extensions: &["abnf"],
        parent: None,
    },
    FormatEntry {
        name: "agda",
        extensions: &["agda"],
        parent: None,
    },
    FormatEntry {
        name: "antlr4",
        extensions: &["g4"],
        parent: None,
    },
    FormatEntry {
        name: "apex",
        extensions: &["cls", "trigger", "apex"],
        parent: None,
    },
    FormatEntry {
        name: "aql",
        extensions: &["aql"],
        parent: None,
    },
    FormatEntry {
        name: "armasm",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "awk",
        extensions: &["awk"],
        parent: None,
    },
    FormatEntry {
        name: "bicep",
        extensions: &["bicep"],
        parent: None,
    },
    FormatEntry {
        name: "bnf",
        extensions: &["bnf"],
        parent: None,
    },
    FormatEntry {
        name: "cfscript",
        extensions: &["cfc"],
        parent: None,
    },
    FormatEntry {
        name: "cfml",
        extensions: &["cfm"],
        parent: None,
    },
    FormatEntry {
        name: "cmake",
        extensions: &["cmake"],
        parent: None,
    },
    FormatEntry {
        name: "cobol",
        extensions: &["cob", "cbl", "cpy", "cobol"],
        parent: None,
    },
    FormatEntry {
        name: "csv",
        extensions: &["csv"],
        parent: None,
    },
    FormatEntry {
        name: "cypher",
        extensions: &["cypher", "cyp"],
        parent: None,
    },
    FormatEntry {
        name: "dhall",
        extensions: &["dhall"],
        parent: None,
    },
    FormatEntry {
        name: "dns-zone-file",
        extensions: &["zone", "arpa"],
        parent: None,
    },
    FormatEntry {
        name: "dot",
        extensions: &["dot", "gv"],
        parent: None,
    },
    FormatEntry {
        name: "ebnf",
        extensions: &["ebnf"],
        parent: None,
    },
    FormatEntry {
        name: "editorconfig",
        extensions: &["editorconfig"],
        parent: None,
    },
    FormatEntry {
        name: "excel-formula",
        extensions: &["xlsx", "xls"],
        parent: None,
    },
    FormatEntry {
        name: "factor",
        extensions: &["factor"],
        parent: None,
    },
    FormatEntry {
        name: "ftl",
        extensions: &["ftl", "ftlh"],
        parent: None,
    },
    FormatEntry {
        name: "gcode",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "gettext",
        extensions: &["po"],
        parent: None,
    },
    FormatEntry {
        name: "gml",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "go-module",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "hcl",
        extensions: &["tf", "hcl", "tfvars"],
        parent: None,
    },
    FormatEntry {
        name: "hlsl",
        extensions: &["hlsl", "hlsli", "fx", "fxh", "cginc"],
        parent: None,
    },
    FormatEntry {
        name: "idris",
        extensions: &["idr"],
        parent: None,
    },
    FormatEntry {
        name: "ignore",
        extensions: &["gitignore"],
        parent: None,
    },
    FormatEntry {
        name: "jq",
        extensions: &["jq"],
        parent: None,
    },
    FormatEntry {
        name: "json5",
        extensions: &["json5"],
        parent: None,
    },
    FormatEntry {
        name: "kusto",
        extensions: &["kql"],
        parent: None,
    },
    FormatEntry {
        name: "lilypond",
        extensions: &["ly"],
        parent: None,
    },
    FormatEntry {
        name: "linker-script",
        extensions: &["ld"],
        parent: None,
    },
    FormatEntry {
        name: "llvm",
        extensions: &["ll"],
        parent: None,
    },
    FormatEntry {
        name: "log",
        extensions: &["log"],
        parent: None,
    },
    FormatEntry {
        name: "mermaid",
        extensions: &["mmd", "mermaid"],
        parent: None,
    },
    FormatEntry {
        name: "mongodb",
        extensions: &["mongodb"],
        parent: None,
    },
    FormatEntry {
        name: "n1ql",
        extensions: &["n1ql"],
        parent: None,
    },
    FormatEntry {
        name: "odin",
        extensions: &["odin"],
        parent: None,
    },
    FormatEntry {
        name: "openqasm",
        extensions: &["qasm"],
        parent: None,
    },
    FormatEntry {
        name: "plant-uml",
        extensions: &["puml", "plantuml"],
        parent: None,
    },
    FormatEntry {
        name: "powerquery",
        extensions: &["pq"],
        parent: None,
    },
    FormatEntry {
        name: "promql",
        extensions: &["promql"],
        parent: None,
    },
    FormatEntry {
        name: "purescript",
        extensions: &["purs"],
        parent: None,
    },
    FormatEntry {
        name: "qsharp",
        extensions: &["qs"],
        parent: None,
    },
    FormatEntry {
        name: "racket",
        extensions: &["rkt"],
        parent: None,
    },
    FormatEntry {
        name: "regex",
        extensions: &["regex", "regexp"],
        parent: None,
    },
    FormatEntry {
        name: "rego",
        extensions: &["rego"],
        parent: None,
    },
    FormatEntry {
        name: "rescript",
        extensions: &["res"],
        parent: None,
    },
    FormatEntry {
        name: "robotframework",
        extensions: &["robot"],
        parent: None,
    },
    FormatEntry {
        name: "shell-session",
        extensions: &["sh-session"],
        parent: None,
    },
    FormatEntry {
        name: "smali",
        extensions: &["smali"],
        parent: None,
    },
    FormatEntry {
        name: "solidity",
        extensions: &["sol"],
        parent: None,
    },
    FormatEntry {
        name: "sparql",
        extensions: &["rq"],
        parent: None,
    },
    FormatEntry {
        name: "stata",
        extensions: &["do", "ado", "mata"],
        parent: None,
    },
    FormatEntry {
        name: "toml",
        extensions: &["toml"],
        parent: None,
    },
    FormatEntry {
        name: "turtle",
        extensions: &["ttl"],
        parent: None,
    },
    FormatEntry {
        name: "typoscript",
        extensions: &["typoscript"],
        parent: None,
    },
    FormatEntry {
        name: "unrealscript",
        extensions: &["uc"],
        parent: None,
    },
    FormatEntry {
        name: "uri",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "vala",
        extensions: &["vala", "vapi"],
        parent: None,
    },
    FormatEntry {
        name: "wgsl",
        extensions: &["wgsl"],
        parent: None,
    },
    FormatEntry {
        name: "wolfram",
        extensions: &["wl", "nb"],
        parent: None,
    },
    FormatEntry {
        name: "zig",
        extensions: &["zig"],
        parent: None,
    },
];

static EXT_TO_FORMAT: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for entry in SUPPORTED_FORMATS {
        for ext in entry.extensions {
            map.insert(*ext, entry.name);
        }
    }
    map
});

/// O(1) lookup: file extension → format name
pub fn get_format_by_extension(ext: &str) -> Option<&'static str> {
    EXT_TO_FORMAT.get(ext).copied()
}

/// Conventional file names that say what a file is when its extension does
/// not: `Makefile` has none, and `CMakeLists.txt` would otherwise be plain
/// text. A name here wins over the extension. Lockfiles and checksum lists
/// (`go.sum`, `Gemfile.lock`) are left out: they are generated, and the
/// lines they share across modules are not duplication anyone can fix.
pub static FILE_NAMES: &[(&str, &str)] = &[
    (".htaccess", "apacheconf"),
    ("apache2.conf", "apacheconf"),
    ("httpd.conf", "apacheconf"),
    (".bash_aliases", "bash"),
    (".bash_logout", "bash"),
    (".bash_profile", "bash"),
    (".bashrc", "bash"),
    (".profile", "bash"),
    (".zlogin", "bash"),
    (".zprofile", "bash"),
    (".zshenv", "bash"),
    (".zshrc", "bash"),
    ("PKGBUILD", "bash"),
    ("CMakeLists.txt", "cmake"),
    ("Containerfile", "docker"),
    ("Dockerfile", "docker"),
    (".editorconfig", "editorconfig"),
    ("Emakefile", "erlang"),
    ("rebar.config", "erlang"),
    ("go.mod", "go-module"),
    ("go.work", "go-module"),
    ("Jenkinsfile", "groovy"),
    ("BSDmakefile", "makefile"),
    ("GNUmakefile", "makefile"),
    ("Makefile", "makefile"),
    ("makefile", "makefile"),
    ("nginx.conf", "nginx"),
    ("Berksfile", "ruby"),
    ("Brewfile", "ruby"),
    ("Capfile", "ruby"),
    ("Dangerfile", "ruby"),
    ("Fastfile", "ruby"),
    ("Gemfile", "ruby"),
    ("Guardfile", "ruby"),
    ("Podfile", "ruby"),
    ("Rakefile", "ruby"),
    ("Thorfile", "ruby"),
    ("Vagrantfile", "ruby"),
    ("Pipfile", "toml"),
    (".exrc", "vim"),
    (".gvimrc", "vim"),
    (".vimrc", "vim"),
    ("_vimrc", "vim"),
    ("gvimrc", "vim"),
    ("vimrc", "vim"),
];

static NAME_TO_FORMAT: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| FILE_NAMES.iter().copied().collect());

/// O(1) lookup: file name (`Makefile`, `go.mod`) → format name
pub fn get_format_by_file_name(name: &str) -> Option<&'static str> {
    NAME_TO_FORMAT.get(name).copied()
}

/// Shebang detection: first line → format name
pub fn get_format_by_shebang(first_line: &str) -> Option<&'static str> {
    if first_line.contains("python3") || first_line.contains("python") {
        Some("python")
    } else if first_line.contains("node") || first_line.contains("nodejs") {
        Some("javascript")
    } else if first_line.contains("ruby") {
        Some("ruby")
    } else if first_line.contains("bash") || first_line.contains("/sh") {
        Some("bash")
    } else if first_line.contains("perl") {
        Some("perl")
    } else if first_line.contains("php") {
        Some("php")
    } else {
        None
    }
}

/// Returns all supported format names (for --list flag)
pub fn list_formats() -> Vec<&'static str> {
    SUPPORTED_FORMATS.iter().map(|e| e.name).collect()
}

static SYNONYMS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert("node", "javascript");
    map.insert("shell", "bash");
    map.insert("zsh", "bash");
    map.insert("golang", "go");
    map
});

pub fn resolve_format(hint: &str) -> Option<&'static str> {
    let normalized = hint.to_lowercase();
    if let Some(name) = SYNONYMS.get(normalized.as_str()) {
        return Some(*name);
    }
    if let Some(name) = get_format_by_extension(&normalized) {
        return Some(name);
    }
    SUPPORTED_FORMATS
        .iter()
        .find(|entry| entry.name == normalized)
        .map(|entry| entry.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_extension_resolves() {
        assert_eq!(get_format_by_extension("js"), Some("javascript"));
    }

    #[test]
    fn ts_extension_resolves() {
        assert_eq!(get_format_by_extension("ts"), Some("typescript"));
    }

    #[test]
    fn unknown_extension_returns_none() {
        assert_eq!(get_format_by_extension("unknown_xyz_abc"), None);
    }

    #[test]
    fn python_shebang_resolves() {
        assert_eq!(
            get_format_by_shebang("#!/usr/bin/env python3"),
            Some("python")
        );
    }

    #[test]
    fn conventional_file_names_resolve() {
        assert_eq!(get_format_by_file_name("Makefile"), Some("makefile"));
        assert_eq!(get_format_by_file_name("Dockerfile"), Some("docker"));
        assert_eq!(get_format_by_file_name("go.mod"), Some("go-module"));
        assert_eq!(get_format_by_file_name("CMakeLists.txt"), Some("cmake"));
        assert_eq!(get_format_by_file_name("Gemfile"), Some("ruby"));
        // Generated lists are not source.
        assert_eq!(get_format_by_file_name("go.sum"), None);
        assert_eq!(get_format_by_file_name("Gemfile.lock"), None);
    }

    #[test]
    fn generated_and_ambiguous_extensions_stay_unmapped() {
        // Sorbet stubs from tapioca, GNU as and Go assembler sources, slicer
        // output, and the XML formats behind `.gml` and `.csl`.
        for ext in ["rbi", "s", "S", "gcode", "gco", "gml", "csl"] {
            assert_eq!(get_format_by_extension(ext), None, "{ext}");
        }
    }

    #[test]
    fn formats_that_had_no_extension_now_have_one() {
        for (ext, format) in [
            ("ex", "elixir"),
            ("exs", "elixir"),
            ("nix", "nix"),
            ("nim", "nim"),
            ("vala", "vala"),
            ("odin", "odin"),
            ("bat", "batch"),
            ("mk", "makefile"),
            ("dockerfile", "docker"),
            ("rst", "rest"),
            ("zsh", "bash"),
        ] {
            assert_eq!(get_format_by_extension(ext), Some(format), "{ext}");
        }
    }

    #[test]
    fn an_extension_or_file_name_belongs_to_one_format() {
        // The lookup maps keep the last entry, so a second owner would win
        // silently.
        let mut owners: HashMap<&str, &str> = HashMap::new();
        for entry in SUPPORTED_FORMATS {
            for ext in entry.extensions {
                let first = owners.insert(ext, entry.name);
                assert_eq!(
                    first, None,
                    ".{ext} is claimed by {first:?} and {}",
                    entry.name
                );
            }
        }
        let mut names: HashMap<&str, &str> = HashMap::new();
        for (name, format) in FILE_NAMES {
            let first = names.insert(name, format);
            assert_eq!(first, None, "{name} is claimed twice");
            assert!(
                list_formats().contains(format),
                "{name}: unknown format {format}"
            );
        }
    }

    #[test]
    fn formats_without_files_of_their_own_are_the_known_ones() {
        // A new format needs an extension or a file name, or a line here
        // that says why it has none.
        let mut without: Vec<&str> = SUPPORTED_FORMATS
            .iter()
            .filter(|entry| entry.extensions.is_empty())
            .filter(|entry| FILE_NAMES.iter().all(|(_, format)| *format != entry.name))
            .map(|entry| entry.name)
            .collect();
        without.sort_unstable();
        assert_eq!(
            without,
            [
                "arff",       // data, its rows repeat by design
                "armasm",     // `.s` is mostly GNU as, where `;` is no comment
                "asm6502",    // `.asm` went to nasm, `.s` is mostly GNU as
                "clike",      // helper grammar, not a file type
                "comments",   // helper grammar
                "csp",        // an HTTP header value
                "css-extras", // helper grammar
                "flow",       // Flow code lives in `.js` files
                "gcode",      // generated by slicers and CAM tools
                "gedcom",     // data, its records repeat by design
                "git",        // git command output
                "gml",        // `.gml` is mostly Geography Markup Language XML
                "hpkp",       // an HTTP header value
                "hsts",       // an HTTP header value
                "ichigojam",  // typed into the machine, no file extension
                "keymap",     // QMK keymaps are C files
                "parigp",     // `.gp` is gnuplot's extension too
                "qore",       // `.q` belongs to q
                "roboconf",   // `.graph` and `.instances` are too generic
                "uri",        // a URI, not a file
                "xeora",      // no established extension
            ]
        );
    }

    #[test]
    fn list_formats_has_at_least_100_entries() {
        assert!(list_formats().len() >= 100);
    }

    #[test]
    fn supported_formats_has_224_entries() {
        assert_eq!(SUPPORTED_FORMATS.len(), 224);
    }

    #[test]
    fn resolve_format_js_extension() {
        assert_eq!(resolve_format("js"), Some("javascript"));
    }

    #[test]
    fn resolve_format_shell_synonym() {
        assert_eq!(resolve_format("shell"), Some("bash"));
    }

    #[test]
    fn resolve_format_golang_synonym() {
        assert_eq!(resolve_format("golang"), Some("go"));
    }

    #[test]
    fn resolve_format_unknown_returns_none() {
        assert_eq!(resolve_format("unknownxyz"), None);
    }

    #[test]
    fn resolve_format_python_name() {
        assert_eq!(resolve_format("python"), Some("python"));
    }

    #[test]
    fn empty_source_tokenize_returns_empty() {
        use crate::tokenizer::{Mode, tokenize};
        let result = std::panic::catch_unwind(|| tokenize("javascript", "", Mode::Mild));
        assert!(result.is_ok());
    }
}
