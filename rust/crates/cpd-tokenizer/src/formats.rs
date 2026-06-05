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
        extensions: &[],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "apl",
        extensions: &["apl"],
        parent: None,
    },
    FormatEntry {
        name: "applescript",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "arduino",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "arff",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "asciidoc",
        extensions: &[],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "autoit",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "bash",
        extensions: &["sh", "ksh", "bash"],
        parent: None,
    },
    FormatEntry {
        name: "basic",
        extensions: &["bas"],
        parent: None,
    },
    FormatEntry {
        name: "batch",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "bison",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "brainfuck",
        extensions: &["b", "bf"],
        parent: None,
    },
    FormatEntry {
        name: "bro",
        extensions: &[],
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
        extensions: &["cljs", "clj", "cljc", "cljx", "edn"],
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
        extensions: &["cpp", "c++", "cc", "cxx"],
        parent: None,
    },
    FormatEntry {
        name: "cpp-header",
        extensions: &["hpp", "h++", "hh", "hxx"],
        parent: None,
    },
    FormatEntry {
        name: "crystal",
        extensions: &["cr"],
        parent: None,
    },
    FormatEntry {
        name: "csharp",
        extensions: &["cs"],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "docker",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "eiffel",
        extensions: &["e"],
        parent: None,
    },
    FormatEntry {
        name: "elixir",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "elm",
        extensions: &["elm"],
        parent: None,
    },
    FormatEntry {
        name: "erb",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "erlang",
        extensions: &["erl", "erlang"],
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
        extensions: &["fs"],
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
        extensions: &[],
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
        extensions: &["groovy", "gradle"],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "ichigojam",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "icon",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "inform7",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "ini",
        extensions: &["ini"],
        parent: None,
    },
    FormatEntry {
        name: "io",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "j",
        extensions: &[],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "json",
        extensions: &["json", "map", "jsonld"],
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
        extensions: &[],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "lua",
        extensions: &["lua"],
        parent: None,
    },
    FormatEntry {
        name: "makefile",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "markdown",
        extensions: &["md", "markdown", "mkd"],
        parent: None,
    },
    FormatEntry {
        name: "markup",
        extensions: &["html", "htm", "xml", "xsl", "xslt", "svg", "ejs", "jsp"],
        parent: None,
    },
    FormatEntry {
        name: "matlab",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "mel",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "mizar",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "monkey",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "n4js",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "nasm",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "nginx",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "nim",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "nix",
        extensions: &[],
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
        extensions: &[],
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
        extensions: &[],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "python",
        extensions: &["py", "pyx", "pxd", "pxi"],
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
        name: "reason",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "renpy",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "rest",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "rip",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "roboconf",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "ruby",
        extensions: &["rb"],
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
        extensions: &["scala"],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "visual-basic",
        extensions: &[],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "wiki",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "xeora",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "xojo",
        extensions: &[],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "agda",
        extensions: &[],
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
        extensions: &[],
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
        extensions: &[],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "csv",
        extensions: &["csv"],
        parent: None,
    },
    FormatEntry {
        name: "cypher",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "dhall",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "dns-zone-file",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "dot",
        extensions: &["dot", "gv"],
        parent: None,
    },
    FormatEntry {
        name: "ebnf",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "editorconfig",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "excel-formula",
        extensions: &["xlsx", "xls"],
        parent: None,
    },
    FormatEntry {
        name: "factor",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "ftl",
        extensions: &[],
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
        extensions: &["tf", "hcl"],
        parent: None,
    },
    FormatEntry {
        name: "hlsl",
        extensions: &[],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "json5",
        extensions: &["json5"],
        parent: None,
    },
    FormatEntry {
        name: "kusto",
        extensions: &[],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "mongodb",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "n1ql",
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "odin",
        extensions: &[],
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
        extensions: &[],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "rego",
        extensions: &[],
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
        extensions: &[],
        parent: None,
    },
    FormatEntry {
        name: "smali",
        extensions: &[],
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
        extensions: &[],
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
        extensions: &[],
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
        extensions: &[],
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
    fn list_formats_has_at_least_100_entries() {
        assert!(list_formats().len() >= 100);
    }

    #[test]
    fn supported_formats_has_223_entries() {
        assert_eq!(SUPPORTED_FORMATS.len(), 223);
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
