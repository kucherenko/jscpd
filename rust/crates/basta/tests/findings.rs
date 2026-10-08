//! What basta reports for small projects written to disk: which files,
//! exports, symbols and imports come back unused, and which do not.
//!
//! Every test here goes through the same front door as the binary —
//! `analyze::run` over a directory — so it holds whatever the analyzers,
//! the resolver and the graph do internally.

use basta::analyze::run;
use basta::config::BastaConfig;
use cpd_core::deadcode::{Category, Report, SymbolKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A project written under the system temp directory, removed on drop.
struct Project(PathBuf);

impl Project {
    fn new(files: &[(&str, &[u8])]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "basta-findings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&root).ok();
        for (path, bytes) in files {
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, bytes).unwrap();
        }
        Self(root)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn everything() -> BastaConfig {
    BastaConfig {
        categories: Category::ALL.to_vec(),
        min_confidence: 0,
        ..BastaConfig::default()
    }
}

/// Scan text files with every category on and no confidence floor.
fn scan(files: &[(&str, &str)]) -> Report {
    let bytes: Vec<(&str, &[u8])> = files.iter().map(|(p, s)| (*p, s.as_bytes())).collect();
    scan_bytes(&bytes, everything())
}

fn scan_bytes(files: &[(&str, &[u8])], config: BastaConfig) -> Report {
    let project = Project::new(files);
    let config = BastaConfig {
        paths: vec![project.path().to_path_buf()],
        ..config
    };
    run(&config).report
}

/// `(category, path, name)` of every finding, for failure messages.
fn listed(report: &Report) -> Vec<(Category, &str, &str)> {
    report
        .findings
        .iter()
        .map(|f| (f.category, f.path.as_str(), f.name.as_str()))
        .collect()
}

fn reported(report: &Report, category: Category, path: &str, name: &str) -> bool {
    report
        .findings
        .iter()
        .any(|f| f.category == category && f.path == path && f.name == name)
}

fn file_unused(report: &Report, path: &str) -> bool {
    reported(report, Category::UnusedFile, path, "")
}

#[track_caller]
fn assert_reported(report: &Report, category: Category, path: &str, name: &str) {
    assert!(
        reported(report, category, path, name),
        "expected {category:?} {path} {name} in {:#?}",
        listed(report)
    );
}

#[track_caller]
fn assert_not_reported(report: &Report, path: &str, name: &str) {
    let hits: Vec<_> = listed(report)
        .into_iter()
        .filter(|(_, p, n)| *p == path && *n == name)
        .collect();
    assert!(
        hits.is_empty(),
        "{path} {name} must not be reported: {hits:?}"
    );
}

#[track_caller]
fn assert_file_used(report: &Report, path: &str) {
    assert!(
        !file_unused(report, path),
        "{path} is reached, yet reported unused: {:#?}",
        listed(report)
    );
}

// ── JavaScript and TypeScript resolution ────────────────────────────────

#[test]
fn an_esm_js_specifier_reaches_the_typescript_file_it_was_written_against() {
    let report = scan(&[
        (
            "src/index.ts",
            "import { a } from './a.js';\nimport { b } from './b.mjs';\na(); b();\n",
        ),
        (
            "src/a.ts",
            "export function a() {}\nexport function spare() {}\n",
        ),
        ("src/b.mts", "export function b() {}\n"),
    ]);
    assert_file_used(&report, "src/a.ts");
    assert_file_used(&report, "src/b.mts");
    assert_not_reported(&report, "src/a.ts", "a");
    assert_reported(&report, Category::UnusedExport, "src/a.ts", "spare");
}

#[test]
#[ignore = "known bug: `./x.js` is rewritten to `./x.ts` only, never to `./x.tsx`"]
fn an_esm_js_specifier_reaches_a_tsx_file() {
    let report = scan(&[
        (
            "src/index.ts",
            "import { Button } from './Button.js';\nconsole.log(Button);\n",
        ),
        (
            "src/Button.tsx",
            "export const Button = () => <button />;\n",
        ),
    ]);
    assert_file_used(&report, "src/Button.tsx");
    assert_not_reported(&report, "src/Button.tsx", "Button");
}

#[test]
fn a_tsconfig_paths_alias_reaches_its_target_even_through_extends() {
    let report = scan(&[
        (
            "tsconfig.base.json",
            "{ // comments are allowed\n  \"compilerOptions\": { \"paths\": { \"@lib/*\": [\"./lib/*\"] } },\n}\n",
        ),
        (
            "tsconfig.json",
            "{ \"extends\": \"./tsconfig.base.json\" }\n",
        ),
        (
            "src/index.ts",
            "import { helper } from '@lib/helper';\nhelper();\n",
        ),
        ("lib/helper.ts", "export function helper() {}\n"),
        ("lib/orphan.ts", "export function orphan() {}\n"),
    ]);
    assert_file_used(&report, "lib/helper.ts");
    assert!(
        file_unused(&report, "lib/orphan.ts"),
        "{:#?}",
        listed(&report)
    );
}

/// TypeScript appends `.json` to an `extends` that does not end in it, so
/// `./tsconfig.base` names `tsconfig.base.json`.
#[test]
#[ignore = "known bug: an extends like `./tsconfig.base` is taken to have the extension `base` and is not found"]
fn a_dotted_extends_without_json_is_followed() {
    let report = scan(&[
        (
            "tsconfig.base.json",
            "{ \"compilerOptions\": { \"paths\": { \"@lib/*\": [\"./lib/*\"] } } }\n",
        ),
        ("tsconfig.json", "{ \"extends\": \"./tsconfig.base\" }\n"),
        (
            "src/index.ts",
            "import { helper } from '@lib/helper';\nhelper();\n",
        ),
        ("lib/helper.ts", "export function helper() {}\n"),
    ]);
    assert_file_used(&report, "lib/helper.ts");
}

#[test]
fn a_vite_alias_in_either_form_reaches_its_target() {
    let report = scan(&[
        (
            "vite.config.ts",
            "import { defineConfig } from 'vite';\nexport default defineConfig({\n  resolve: { alias: { '@': './src', $shared: './shared' } },\n});\n",
        ),
        (
            "src/main.ts",
            "import { a } from '@/a';\nimport { s } from '$shared/s';\na(); s();\n",
        ),
        ("src/a.ts", "export function a() {}\n"),
        ("shared/s.ts", "export function s() {}\n"),
    ]);
    assert_file_used(&report, "src/a.ts");
    assert_file_used(&report, "shared/s.ts");

    let report = scan(&[
        (
            "vite.config.js",
            "export default {\n  resolve: { alias: [ { find: '~', replacement: './src' } ] },\n};\n",
        ),
        ("src/main.ts", "import { a } from '~/a';\na();\n"),
        ("src/a.ts", "export function a() {}\n"),
    ]);
    assert_file_used(&report, "src/a.ts");
}

#[test]
fn a_workspace_package_name_reaches_its_exported_source() {
    let report = scan(&[
        (
            "package.json",
            "{ \"name\": \"root\", \"private\": true, \"workspaces\": [\"packages/*\", \"apps/*\"] }\n",
        ),
        (
            "packages/ui/package.json",
            "{ \"name\": \"@acme/ui\", \"exports\": { \".\": { \"types\": \"./src/index.ts\", \"import\": \"./dist/index.mjs\" }, \"./*\": \"./src/*.ts\" } }\n",
        ),
        (
            "packages/ui/src/index.ts",
            "export function Button() {}\nexport function NeverUsed() {}\n",
        ),
        ("packages/ui/src/theme.ts", "export const theme = 1;\n"),
        ("packages/ui/lib/dead.js", "export const dead = 1;\n"),
        (
            "apps/web/src/main.ts",
            "import { Button } from '@acme/ui';\nimport { theme } from '@acme/ui/theme';\nButton(theme);\n",
        ),
    ]);
    assert_file_used(&report, "packages/ui/src/theme.ts");
    assert_not_reported(&report, "packages/ui/src/index.ts", "Button");
    // A source file the package neither exports nor imports is still dead.
    assert!(
        file_unused(&report, "packages/ui/lib/dead.js"),
        "{:#?}",
        listed(&report)
    );
}

#[test]
fn glob_and_template_imports_keep_the_files_they_can_load() {
    let report = scan(&[
        (
            "src/main.ts",
            "const pages = import.meta.glob(['./pages/*.ts', '!./pages/skip.ts']);\nconst lang = navigator.language;\nimport(`./locales/${lang}.js`).then(console.log);\nconsole.log(pages);\n",
        ),
        ("src/pages/home.ts", "export const home = 1;\n"),
        ("src/locales/en.js", "export default {};\n"),
        ("src/locales/archive/old.js", "export default {};\n"),
    ]);
    assert_file_used(&report, "src/pages/home.ts");
    assert_file_used(&report, "src/locales/en.js");
    assert!(
        file_unused(&report, "src/locales/archive/old.js"),
        "`*` stays within one segment: {:#?}",
        listed(&report)
    );
}

#[test]
fn commonjs_requires_and_exports_are_followed() {
    let report = scan(&[
        (
            "index.cjs",
            "const { used } = require('./lib.cjs');\nconst util = require(`./util`);\nused(util);\n",
        ),
        (
            "lib.cjs",
            "function used() {}\nfunction spare() {}\nmodule.exports = { used, spare };\n",
        ),
        ("util.js", "module.exports = function util() {};\n"),
    ]);
    assert_file_used(&report, "lib.cjs");
    assert_file_used(&report, "util.js");
    assert_not_reported(&report, "lib.cjs", "used");
    assert_reported(&report, Category::UnusedExport, "lib.cjs", "spare");
}

#[test]
fn unused_imports_and_private_symbols_are_reported_and_used_ones_are_not() {
    let report = scan(&[
        (
            "src/index.ts",
            "import { used, unused } from './lib';\nfunction helper() { return used(); }\nfunction orphan() {}\nexport const value = helper();\n",
        ),
        (
            "src/lib.ts",
            "export function used() {}\nexport function unused() {}\n",
        ),
    ]);
    assert_reported(&report, Category::UnusedImport, "src/index.ts", "unused");
    assert_reported(&report, Category::UnusedSymbol, "src/index.ts", "orphan");
    assert_not_reported(&report, "src/index.ts", "used");
    assert_not_reported(&report, "src/index.ts", "helper");
}

#[test]
fn a_tsdoc_link_is_a_use_of_what_it_names() {
    // TypeScript counts `{@link Cart}` as a use of `import type { Cart }`
    // and of a local declaration; a URL in a link names nothing.
    let report = scan(&[
        (
            "src/index.ts",
            "import type { Cart, Item, Price, Unlinked } from './types';
\
             function format(): string { return ''; }
\
             /**\n * Totals a {@link Cart}, see {@linkcode Item.price} and\n\
             * {@linkplain Price | the price}, {@link format} and\n\
             * {@link https://example.com Unlinked}.\n */\n\
             export function total(): number { return 0; }\n",
        ),
        (
            "src/types.ts",
            "export interface Cart { items: Item[] }\n\
             export interface Item { price: Price }\n\
             export type Price = number;\n\
             export interface Unlinked { id: string }\n",
        ),
    ]);
    for name in ["Cart", "Item", "Price", "format"] {
        assert_not_reported(&report, "src/index.ts", name);
    }
    assert_reported(&report, Category::UnusedImport, "src/index.ts", "Unlinked");
}

#[test]
fn a_finding_names_the_kind_of_declaration_it_is() {
    let report = scan(&[(
        "src/index.ts",
        "function a() {}\nclass B {}\nconst c = 1;\ntype D = string;\ninterface E {}\nenum F { X }\nconst g = () => 1;\nfunction used() {}\nused(); used();\n",
    )]);
    let kind_of = |name: &str| {
        report
            .findings
            .iter()
            .find(|f| f.path == "src/index.ts" && f.name == name)
            .unwrap_or_else(|| panic!("{name} unreported: {:#?}", listed(&report)))
            .symbol_kind
    };
    assert_eq!(kind_of("a"), Some(SymbolKind::Function));
    assert_eq!(kind_of("B"), Some(SymbolKind::Class));
    assert_eq!(kind_of("c"), Some(SymbolKind::Variable));
    assert_eq!(kind_of("D"), Some(SymbolKind::TypeAlias));
    assert_eq!(kind_of("E"), Some(SymbolKind::Interface));
    assert_eq!(kind_of("F"), Some(SymbolKind::Enum));
    assert_eq!(
        kind_of("g"),
        Some(SymbolKind::Function),
        "an arrow bound to a const reads as a function"
    );
    assert_not_reported(&report, "src/index.ts", "used");
}

/// A string that spells the last two segments of a file's path is weak
/// evidence the file is loaded by name: its unused-file finding is less
/// certain than one for a file nothing names. Prose, URLs and header values
/// are not paths.
#[test]
fn a_path_shaped_string_lowers_the_confidence_that_a_file_is_unused() {
    let report = scan(&[
        (
            "src/index.ts",
            "register('runtime/handlers/island');\nregister('#app/components/nuxt-link');\nfetch('https://example.com/api/client');\nheader('text/html; charset=utf-8');\n",
        ),
        ("src/runtime/handlers/island.ts", "export default 1;\n"),
        ("src/components/nuxt-link.ts", "export default 1;\n"),
        ("src/api/client.ts", "export default 1;\n"),
        ("src/text/html.ts", "export default 1;\n"),
        ("src/runtime/handlers/plain.ts", "export default 1;\n"),
    ]);
    let confidence = |path: &str| {
        report
            .findings
            .iter()
            .find(|f| f.category == Category::UnusedFile && f.path == path)
            .unwrap_or_else(|| panic!("{path} unreported: {:#?}", listed(&report)))
            .confidence
    };
    let baseline = confidence("src/runtime/handlers/plain.ts");
    assert!(confidence("src/runtime/handlers/island.ts") < baseline);
    assert!(confidence("src/components/nuxt-link.ts") < baseline);
    assert_eq!(
        confidence("src/api/client.ts"),
        baseline,
        "a URL is not a path"
    );
    assert_eq!(
        confidence("src/text/html.ts"),
        baseline,
        "nor is a header value"
    );
}

// ── JSX factories ──────────────────────────────────────────────────────

/// Under the classic runtime `<div />` compiles to `React.createElement`,
/// so the `React` import is what the JSX uses.
#[test]
#[ignore = "known bug: a classic-runtime React import used only by JSX is reported unused"]
fn a_classic_react_import_is_used_by_the_jsx() {
    let report = scan(&[
        (
            "src/index.jsx",
            "import React from 'react';\nexport function App() { return <div>hi</div>; }\n",
        ),
        (
            "tsconfig.json",
            "{ \"compilerOptions\": { \"jsx\": \"react\" } }\n",
        ),
    ]);
    assert_not_reported(&report, "src/index.jsx", "React");
}

/// `/** @jsx h */` names the factory the JSX in this file compiles to.
#[test]
#[ignore = "known bug: a JSX pragma factory import used only by JSX is reported unused"]
fn a_jsx_pragma_factory_import_is_used_by_the_jsx() {
    let report = scan(&[(
        "src/index.jsx",
        "/** @jsx h */\nimport { h } from 'preact';\nexport const View = () => <p>view</p>;\n",
    )]);
    assert_not_reported(&report, "src/index.jsx", "h");
}

#[test]
fn an_import_that_jsx_names_as_a_component_is_used() {
    let report = scan(&[
        (
            "src/index.tsx",
            "import { Card, Unused } from './Card';\nexport const App = () => <Card.Body><Card /></Card.Body>;\n",
        ),
        (
            "src/Card.tsx",
            "export const Card = () => null;\nexport const Unused = 1;\n",
        ),
    ]);
    assert_not_reported(&report, "src/index.tsx", "Card");
    assert_reported(&report, Category::UnusedImport, "src/index.tsx", "Unused");
}

// ── components ─────────────────────────────────────────────────────────

#[test]
fn a_component_used_only_by_its_template_is_alive() {
    let report = scan(&[
        (
            "src/App.vue",
            "<template>\n  <data-grid :rows=\"rows\" />\n</template>\n<script setup lang=\"ts\">\nimport DataGrid from './DataGrid.vue';\nimport Unused from './Unused.vue';\nconst rows = [];\n</script>\n",
        ),
        (
            "src/main.ts",
            "import App from './App.vue';\nconsole.log(App);\n",
        ),
        ("src/DataGrid.vue", "<template><table /></template>\n"),
        ("src/Unused.vue", "<template><p /></template>\n"),
        ("src/Orphan.vue", "<template><p /></template>\n"),
    ]);
    assert_not_reported(&report, "src/App.vue", "DataGrid");
    assert_reported(&report, Category::UnusedImport, "src/App.vue", "Unused");
    assert_file_used(&report, "src/DataGrid.vue");
    assert!(
        file_unused(&report, "src/Orphan.vue"),
        "{:#?}",
        listed(&report)
    );
}

#[test]
fn an_astro_client_script_and_svelte_stores_count_as_uses() {
    let report = scan(&[
        (
            "src/pages/index.astro",
            "---\nimport Card from '../components/Card.astro';\nimport { unused } from '../lib/unused';\n---\n<Card />\n<script>\nimport '../client/boot';\n</script>\n",
        ),
        ("src/components/Card.astro", "<div />\n"),
        ("src/client/boot.ts", "console.log('boot');\n"),
        ("src/lib/unused.ts", "export const unused = 1;\n"),
        (
            "src/routes/+page.svelte",
            "<script>\nimport { settings } from '$lib/settings';\nimport Spare from '$lib/Spare.svelte';\n</script>\n<p class={$settings.theme}>hi</p>\n",
        ),
        ("src/lib/settings.ts", "export const settings = {};\n"),
        ("src/lib/Spare.svelte", "<p />\n"),
        ("svelte.config.js", "export default { kit: {} };\n"),
        (
            "package.json",
            "{ \"devDependencies\": { \"@sveltejs/kit\": \"2\", \"astro\": \"4\" } }\n",
        ),
    ]);
    assert_file_used(&report, "src/client/boot.ts");
    assert_file_used(&report, "src/components/Card.astro");
    assert_not_reported(&report, "src/pages/index.astro", "Card");
    assert_reported(
        &report,
        Category::UnusedImport,
        "src/pages/index.astro",
        "unused",
    );
    assert_not_reported(&report, "src/routes/+page.svelte", "settings");
    assert_reported(
        &report,
        Category::UnusedImport,
        "src/routes/+page.svelte",
        "Spare",
    );
}

// ── Python ─────────────────────────────────────────────────────────────

#[test]
fn a_python_src_layout_resolves_absolute_imports_and_honours_dunder_all() {
    let report = scan(&[
        (
            "pyproject.toml",
            "[project]\nname = \"tool\"\n\n[project.scripts]\ntool = \"tool.cli:main\"\n",
        ),
        (
            "src/tool/__init__.py",
            "from tool.core import run\n\n__all__ = ['run']\n",
        ),
        (
            "src/tool/cli.py",
            "from tool import run\nfrom tool.util import fmt as _fmt\nimport os, sys  # noqa: F401\n\n\ndef main():\n    run()\n",
        ),
        (
            "src/tool/core.py",
            "def run():\n    return _private()\n\n\ndef _private():\n    pass\n\n\ndef never_called():\n    pass\n",
        ),
        ("src/tool/util.py", "def fmt():\n    pass\n"),
        ("src/tool/orphan.py", "def lonely():\n    pass\n"),
    ]);
    assert_not_reported(&report, "src/tool/cli.py", "main");
    assert_not_reported(&report, "src/tool/core.py", "run");
    assert_not_reported(&report, "src/tool/core.py", "_private");
    assert_reported(
        &report,
        Category::UnusedExport,
        "src/tool/core.py",
        "never_called",
    );
    assert_reported(&report, Category::UnusedImport, "src/tool/cli.py", "_fmt");
    assert_not_reported(&report, "src/tool/cli.py", "os");
    assert!(
        file_unused(&report, "src/tool/orphan.py"),
        "{:#?}",
        listed(&report)
    );
}

#[test]
fn python_relative_imports_and_dynamic_access_are_respected() {
    let report = scan(&[
        ("app/__init__.py", ""),
        (
            "app/__main__.py",
            "from . import handlers\nfrom .models import User\n\nprint(User, handlers)\n",
        ),
        (
            "app/handlers.py",
            "import importlib\n\n\ndef load(name):\n    return getattr(importlib.import_module(name), 'go')\n\n\ndef unused_handler():\n    pass\n",
        ),
        (
            "app/models.py",
            "from typing import TYPE_CHECKING\nif TYPE_CHECKING:\n    from .handlers import load\n\n\nclass User:\n    def method(self) -> \"load\":\n        pass\n\n\ndef spare_model():\n    pass\n",
        ),
    ]);
    assert_file_used(&report, "app/handlers.py");
    assert_file_used(&report, "app/models.py");
    assert_not_reported(&report, "app/models.py", "User");
    assert_not_reported(&report, "app/models.py", "load");
    // `getattr` on a module makes the file's names reachable at runtime, so
    // its unused-looking function is not reported; a plain file's is.
    assert_not_reported(&report, "app/handlers.py", "unused_handler");
    assert_reported(
        &report,
        Category::UnusedExport,
        "app/models.py",
        "spare_model",
    );
}

// ── hostile files ──────────────────────────────────────────────────────

#[test]
fn a_file_that_is_not_utf8_is_counted_as_unparsed_without_stopping_the_run() {
    let report = scan_bytes(
        &[
            ("src/index.ts", b"import { a } from './a';\na();\n"),
            (
                "src/a.ts",
                b"export function a() {}\nexport function spare() {}\n",
            ),
            ("src/latin1.ts", b"export const caf\xe9 = '\xff\xfe';\n"),
        ],
        everything(),
    );
    assert_eq!(report.statistics.files, 3);
    assert_eq!(report.statistics.unparsed, 1, "{:?}", report.statistics);
    assert!(
        report
            .statistics
            .unparsed_files
            .iter()
            .any(|p| p.ends_with("latin1.ts")),
        "{:?}",
        report.statistics.unparsed_files
    );
    assert_reported(&report, Category::UnusedExport, "src/a.ts", "spare");
    // Nothing can be said about what an unreadable file declares.
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.path != "src/latin1.ts" || f.category == Category::UnusedFile),
        "{:#?}",
        listed(&report)
    );
}

#[test]
fn a_file_that_does_not_parse_does_not_make_its_imports_unused() {
    let report = scan(&[
        ("src/index.ts", "import { a } from './a';\na();\n"),
        (
            "src/a.ts",
            "import { b } from './b';\nexport function a( { b(); \n",
        ),
        ("src/b.ts", "export function b() {}\n"),
    ]);
    assert!(report.statistics.unparsed >= 1, "{:?}", report.statistics);
    assert_not_reported(&report, "src/b.ts", "b");
}

#[test]
fn crlf_bom_and_multibyte_sources_report_the_right_lines() {
    let report = scan(&[
        (
            "src/index.ts",
            "\u{feff}import { naïve } from './ünï';\r\nnaïve();\r\n",
        ),
        (
            "src/ünï.ts",
            "// ✓ комментарий\r\nexport function naïve() {}\r\n\r\nexport function 未使用() {}\r\n",
        ),
    ]);
    assert_file_used(&report, "src/ünï.ts");
    assert_not_reported(&report, "src/ünï.ts", "naïve");
    let finding = report
        .findings
        .iter()
        .find(|f| f.name == "未使用")
        .unwrap_or_else(|| panic!("{:#?}", listed(&report)));
    assert_eq!(finding.category, Category::UnusedExport);
    assert_eq!(finding.start.line, 4, "CRLF and multibyte text keep lines");
}

#[test]
fn an_empty_project_and_empty_files_report_nothing_surprising() {
    let report = scan(&[]);
    assert!(report.findings.is_empty());
    assert_eq!(report.statistics.files, 0);

    let report = scan(&[("index.ts", ""), ("empty.py", "")]);
    assert_eq!(report.statistics.unparsed, 0, "{:?}", report.statistics);
    assert_not_reported(&report, "index.ts", "");
}

// ── more resolution shapes ─────────────────────────────────────────────

#[test]
fn re_exports_carry_use_through_a_barrel() {
    let report = scan(&[
        (
            "src/main.ts",
            "import { a, ns, fromMore } from './barrel';\nconsole.log(a, ns, fromMore);\n",
        ),
        (
            "src/barrel.ts",
            "export { a, b as bee } from './lib';\nexport * from './more';\nexport * as ns from './ns';\n",
        ),
        (
            "src/lib.ts",
            "export function a() {}\nexport function b() {}\nexport function c() {}\n",
        ),
        ("src/more.ts", "export const fromMore = 1;\n"),
        ("src/ns.ts", "export const inside = 1;\n"),
    ]);
    assert_file_used(&report, "src/more.ts");
    assert_file_used(&report, "src/ns.ts");
    assert_not_reported(&report, "src/lib.ts", "a");
    assert_not_reported(&report, "src/more.ts", "fromMore");
    assert_reported(&report, Category::UnusedExport, "src/lib.ts", "c");
}

#[test]
fn every_shape_of_require_keeps_its_module() {
    let report = scan(&[
        (
            "index.js",
            "const [first] = require('./arr');\nconst { ...rest } = require('./rest');\nconst { a: { b } } = require('./nested');\nconst { picked } = require('./picked');\nconsole.log(first, rest, b, picked);\n",
        ),
        ("arr.js", "module.exports = [1];\n"),
        ("rest.js", "exports.x = 1;\n"),
        ("nested.js", "exports.a = { b: 1 };\n"),
        (
            "picked.js",
            "const base = {};\nfunction picked() {}\nfunction spare() {}\nmodule.exports = { ...base, ['computed']: 1, picked, spare };\n",
        ),
    ]);
    for file in ["arr.js", "rest.js", "nested.js", "picked.js"] {
        assert_file_used(&report, file);
    }
    assert_not_reported(&report, "picked.js", "picked");
    assert_reported(&report, Category::UnusedExport, "picked.js", "spare");
}

#[test]
fn class_members_are_reported_by_whether_anything_reads_them() {
    let report = scan(&[(
        "src/index.ts",
        "export class Store {\n  #secret = 1;\n  #spare = 2;\n  accessor tracked = 3;\n  read() { return this.#secret; }\n  unusedMethod() {}\n}\nconst Anon = class {};\nnew Store().read();\nconsole.log(Anon);\n",
    )]);
    assert_not_reported(&report, "src/index.ts", "#secret");
    assert_not_reported(&report, "src/index.ts", "read");
    assert_reported(&report, Category::UnusedMember, "src/index.ts", "#spare");
    assert_reported(
        &report,
        Category::UnusedMember,
        "src/index.ts",
        "unusedMethod",
    );
    assert_not_reported(&report, "src/index.ts", "Anon");
}

#[test]
fn bundler_configs_name_files_relative_to_the_project() {
    let report = scan(&[
        (
            "config/webpack.config.js",
            "require('./client/index-app.js');\nmodule.exports = { entry: './client/index-app.js' };\n",
        ),
        ("client/index-app.js", "console.log('app');\n"),
        (
            "src/main.ts",
            "import x from '/src/shared/x';\nconst all = import.meta.glob(pattern);\nconst one = import.meta.glob('./views/*.ts');\nconsole.log(x, all, one);\n",
        ),
        ("src/shared/x.ts", "export default 1;\n"),
        ("src/views/a.ts", "export default 1;\n"),
    ]);
    assert_file_used(&report, "client/index-app.js");
    assert_file_used(&report, "src/shared/x.ts");
    assert_file_used(&report, "src/views/a.ts");
}

#[test]
fn package_exports_in_string_form_and_sveltekit_lib_dirs_resolve() {
    let report = scan(&[
        ("package.json", "{ \"workspaces\": [\"packages/*\"] }\n"),
        (
            "packages/core/package.json",
            "{ \"name\": \"core\", \"exports\": \"./src/index.ts\" }\n",
        ),
        ("packages/core/src/index.ts", "export const core = 1;\n"),
        (
            "app/svelte.config.js",
            "export default { kit: { files: { lib: 'src/shared' } } };\n",
        ),
        (
            "app/package.json",
            "{ \"devDependencies\": { \"@sveltejs/kit\": \"2\" } }\n",
        ),
        (
            "app/src/routes/+page.ts",
            "import { core } from 'core';\nimport { util } from '$lib/util';\nexport const load = () => [core, util];\n",
        ),
        ("app/src/shared/util.ts", "export const util = 1;\n"),
    ]);
    assert_file_used(&report, "app/src/shared/util.ts");
    assert_not_reported(&report, "app/src/shared/util.ts", "util");
}

#[test]
fn component_markup_edge_cases_still_read_what_they_name() {
    let report = scan(&[
        (
            "src/main.ts",
            "import './App.svelte';\nimport './Page.vue';\nimport './Shell.astro';\n",
        ),
        (
            "src/App.svelte",
            "<scriptlet>not a script</scriptlet>\n<script>\nimport Child from './Child.svelte';\nimport { props, label, Unused } from './data';\n</script>\n<Child {...props} title={`${label}!`} />\n",
        ),
        ("src/Child.svelte", "<p />\n"),
        (
            "src/data.ts",
            "export const props = {};\nexport const label = '';\nexport const Unused = 1;\n",
        ),
        (
            "src/Page.vue",
            "<template>\n  <Widget.Item v-bind:size=\"size\" v-if=\"shown\" />\n</template>\n<script setup lang=ts>\nimport Widget from './Widget.vue';\nimport { size, shown } from './state';\n</script>\n",
        ),
        ("src/Widget.vue", "<template><i /></template>\n"),
        (
            "src/state.ts",
            "export const size = 1;\nexport const shown = true;\n",
        ),
        (
            "src/Shell.astro",
            "---\nimport Frame from './Frame.astro';\n---\n<Frame />\n<script>const = = ;</script>\n<script src=\"./loader.ts\" />\n",
        ),
        ("src/Frame.astro", "<div />\n"),
        ("src/loader.ts", "export {};\n"),
    ]);
    assert_file_used(&report, "src/Child.svelte");
    assert_not_reported(&report, "src/App.svelte", "props");
    assert_not_reported(&report, "src/App.svelte", "label");
    assert_reported(&report, Category::UnusedImport, "src/App.svelte", "Unused");
    assert_not_reported(&report, "src/Page.vue", "Widget");
    assert_not_reported(&report, "src/Page.vue", "size");
    assert_not_reported(&report, "src/Page.vue", "shown");
    assert_file_used(&report, "src/Frame.astro");
}

#[test]
fn python_edge_forms_of_all_noqa_and_parent_imports() {
    let report = scan(&[
        ("pkg/__init__.py", ""),
        ("pkg/__main__.py", "from .sub.leaf import go\n\ngo()\n"),
        ("pkg/sub/__init__.py", ""),
        (
            "pkg/sub/leaf.py",
            "from .. import shared\nfrom ..helpers import helper  # NOQA\n\n\ndef go():\n    return shared.value\n",
        ),
        ("pkg/shared.py", "value = 1\n"),
        ("pkg/helpers.py", "def helper():\n    pass\n"),
        (
            "pkg/api.py",
            "__all__: list[str] = ['listed']\n__all__ += ('extra',)\n\n\ndef listed():\n    pass\n\n\ndef extra():\n    pass\n\n\ndef unlisted():\n    pass\n",
        ),
        (
            "pkg/generic.py",
            "from .shared import value\n\n\nclass Box[T](dict):\n    pass\n\n\ndef first[T](items: list[T]) -> T:\n    return items[0]\n\n\ndef first():\n    return value\n\n\n@decorators.register.call()\ndef hooked():\n    pass\n\n\nresult = make()()[0]\n",
        ),
    ]);
    assert_file_used(&report, "pkg/shared.py");
    // `# NOQA` keeps the import deliberately: never an unused import.
    assert!(
        !reported(&report, Category::UnusedImport, "pkg/sub/leaf.py", "helper"),
        "{:#?}",
        listed(&report)
    );
    assert_not_reported(&report, "pkg/sub/leaf.py", "shared");
    assert_not_reported(&report, "pkg/generic.py", "value");
}
