# Ignore mechanisms

Each subdirectory contains a clone that one ignore mechanism removes. All
commands run from the repository root with default thresholds (`--min-tokens 50`,
`--min-lines 5`); the `Found N clones.` line is what the console reporter prints.

| Directory | Mechanism | Default scan | With the ignore applied |
|-----------|-----------|--------------|-------------------------|
| `glob/` | `--ignore` file globs | 2 clones | 0 clones |
| `regex/` | `--ignore-pattern` / `ignorePattern` source regions | 1 clone | 0 clones |
| `markers/` | `jscpd:ignore-start` / `jscpd:ignore-end` comments | 1 clone (`plain/`) | 0 clones (`marked/`) |
| `gitignore/` | `.gitignore` (on by default) | 0 clones | 1 clone with `--no-gitignore` |

## `glob/` — exclude whole files

`src/checkout.js` is copied verbatim into `vendor/` and `generated/`.

```bash
jscpd fixtures/ignore-demo/glob
# Found 2 clones.

jscpd fixtures/ignore-demo/glob --ignore "**/vendor/**"
# Found 1 clones.

jscpd fixtures/ignore-demo/glob --ignore "**/vendor/**,**/generated/**"
# Found 0 clones.
```

## `regex/` — exclude a region of every file

`Invoice.cs` and `Order.cs` share a 13-line Apache license header and nothing
else. C# goes through the generic tokenizer, where comments produce tokens, so
the header alone is a clone. The anchored, dot-all expression
`(?s)\A/\*.*?\*/` matches a block comment at the start of the file; matched
text is removed before tokenization.

```bash
jscpd fixtures/ignore-demo/regex
# Found 1 clones.

jscpd fixtures/ignore-demo/regex --ignore-pattern '(?s)\A/\*.*?\*/'
# Found 0 clones.

# The same pattern from a config file (JSON escaping doubles the backslashes):
jscpd fixtures/ignore-demo/regex --config fixtures/ignore-demo/regex/.jscpd.json
# Found 0 clones.

# Config discovery starts from the working directory:
(cd fixtures/ignore-demo/regex && jscpd .)
# Using config from .jscpd.json
# Found 0 clones.

# Weak mode drops every comment from detection, in every language:
jscpd fixtures/ignore-demo/regex --mode weak
# Found 0 clones.
```

Two things to watch. The CLI value is split on commas, so a regex that contains
one must go in the config file, and a regex that fails to compile is skipped
with a warning rather than aborting the scan:

```bash
jscpd fixtures/ignore-demo/regex --ignore-pattern '(?s)\A/\*.{1,3}.*?\*/'
# Warning: --ignore-pattern: invalid regex '(?s)\A/\*.{1' is skipped: error: unclosed counted repetition
# Found 1 clones.
```

JavaScript and TypeScript comments never produce tokens, so a license header in
those files is never part of a clone and needs no pattern.

## `markers/` — exclude a region of one file

`plain/` and `marked/` hold the same two Python modules with a shared generated
block. In `marked/` the block sits between `# jscpd:ignore-start` and
`# jscpd:ignore-end`. Markers must be inside a comment that is valid for the
language and work in every detection mode.

```bash
jscpd fixtures/ignore-demo/markers/plain
# Found 1 clones.

jscpd fixtures/ignore-demo/markers/marked
# Found 0 clones.
```

## `gitignore/` — `.gitignore` is honored by default

`build/emitter.js` is a copy of `src/emitter.js`, and the directory's
`.gitignore` lists `build/` (the copy is force-added to git for this demo).

```bash
jscpd fixtures/ignore-demo/gitignore
# Found 0 clones.

jscpd fixtures/ignore-demo/gitignore --no-gitignore
# Found 1 clones.
```

## Whole directory

```bash
jscpd fixtures/ignore-demo
# Found 4 clones.
```
