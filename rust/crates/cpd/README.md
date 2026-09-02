# jscpd

Copy/paste detector for source code — a fast Rust CLI that finds duplicated
blocks across 223 languages, writes reports in 15 formats and can fail CI when
duplication grows.

This crate is the Rust engine behind [jscpd](https://github.com/kucherenko/jscpd)
(v5+). Full documentation lives at [jscpd.dev](https://jscpd.dev).

## Install

```bash
cargo install jscpd
# or, without compiling (prebuilt release binaries via cargo-binstall):
cargo binstall jscpd
```

`cargo install jscpd` installs **two** binaries with identical behaviour:
`jscpd` and `cpd`. The unrelated `cpd` crate on crates.io is **not** this
project; the crate name is `jscpd`.

Also available on npm (`npm install -g jscpd`), Homebrew (`brew install jscpd`)
and as prebuilt archives on the
[GitHub releases page](https://github.com/kucherenko/jscpd/releases).

## Usage

```bash
jscpd .                                   # scan the current directory
jscpd src/ lib/                           # scan specific paths
jscpd --min-tokens 50 --min-lines 5 .     # tune sensitivity
jscpd --reporters json,html --output report .
jscpd --threshold 5 .                     # exit 1 above 5% duplicated lines
jscpd --blame .                           # annotate clones with git blame
jscpd --list                              # list supported formats
jscpd --help                              # every option
```

### Config file

Options can also come from `.jscpd.json` in the scanned directory (or the
`jscpd` key of `package.json`, or `--config <path>`):

```json
{
  "minTokens": 50,
  "minLines": 5,
  "mode": "mild",
  "format": ["typescript", "javascript", "rust"],
  "ignore": ["**/node_modules/**", "**/dist/**"],
  "reporters": ["console", "json"],
  "output": "report",
  "threshold": 5
}
```

### Reporters

Select one or more with `--reporters` (`-r`):

`console`, `console-full`, `json`, `xml`, `csv`, `html`, `markdown`, `badge`,
`sarif`, `ai`, `xcode`, `threshold`, `silent`, `openmetrics`, `codeclimate`

### Baselines and CI gating

```bash
# Record the current state, then fail only on clones that were not in it.
jscpd --update-baseline --baseline .jscpd-baseline.json .
jscpd --baseline .jscpd-baseline.json --fail-on-new-clones .

# Or compare against a git ref without a stored baseline file
# (the ref's tree is scanned with the same options).
jscpd --baseline-from-ref origin/main --fail-on-new-clones .
```

### MCP server

`jscpd --mcp .` scans the given paths once, then serves the Model Context
Protocol over stdio, exposing `check_duplication`, `get_statistics` and
`check_current_directory` tools to MCP clients (Claude Code, Cursor, ...).

### GitHub Action

```yaml
- uses: kucherenko/jscpd@v5
  with:
    path: .
    threshold: 5
    reporters: console,sarif
```

## Library use

The `jscpd` crate's library target is **not a public API** — it only exists to
share helpers with the crate's tests and may change in any release. For
programmatic use depend on the engine crates instead:

| Crate | Purpose |
|-------|---------|
| [`cpd-finder`](https://crates.io/crates/cpd-finder) | File walking, orchestration, git blame — the entry point |
| [`cpd-core`](https://crates.io/crates/cpd-core) | Detection algorithm (Rabin–Karp rolling hash), data models |
| [`cpd-tokenizer`](https://crates.io/crates/cpd-tokenizer) | Language tokenization (223 formats) |
| [`cpd-reporter`](https://crates.io/crates/cpd-reporter) | Output formatting (15 reporters) |

```rust,no_run
use cpd_finder::orchestrate::{RunConfig, run};

let config = RunConfig {
    paths: vec!["./src".into()],
    min_tokens: 50,
    ..Default::default()
};

let result = run(&config).unwrap();
println!("Found {} clones", result.clones.len());
println!("Analyzed {} files", result.statistics.total.sources);
```

## Links

- Repository: <https://github.com/kucherenko/jscpd>
- Documentation: <https://jscpd.dev>
- Changelog: <https://github.com/kucherenko/jscpd/blob/master/rust/CHANGELOG.md>

## License

MIT
