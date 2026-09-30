# lsp-demo

This demo is a small library-loans module to open in an editor that runs `jscpd --lsp`. Clones are on by default, and the `lsp` section of its `.jscpd.json` turns on three more analyses: similar functions (`ast`), dead code and complexity. Each file then shows what one of them reports. The setup for each editor is in [docs/editors.md](../../docs/editors.md).

Run the commands from this directory, so that the demo's own `.jscpd.json` is the config. The numbers below come from the defaults (`--min-tokens 50`, `--min-lines 5`).

| File | What it holds | Diagnostic |
|---|---|---|
| `src/loans.js` | `daysLate`, copied into `holds.js` | `jscpd/duplicate-code` on line 4 |
| | `renewalsLeft`, built like `holdsAhead` in `holds.js` | `jscpd/similar-function` on line 13 |
| | `loanStatus`, with 19 branches | `jscpd/complex-function` on line 22 |
| `src/holds.js` | the other copy of `daysLate`, and `holdsAhead` | the same clone and pair, seen from this side |
| `src/index.js` | an import of `padLeft` that nothing uses | `unused-import` on line 3, faded |
| `src/format.js` | `formatFine`, which no file imports | `unused-export` on line 7, faded |

## In headless Neovim

This prints what the server publishes for one file. It needs Neovim 0.10 or later and `jscpd` on the `PATH`:

```bash
cd fixtures/lsp-demo
nvim --headless --clean src/loans.js \
  -c "lua vim.lsp.start({ name = 'jscpd', cmd = { 'jscpd', '--lsp' }, root_dir = vim.fn.getcwd() })" \
  -c "lua vim.wait(3000)" \
  -c "lua for _, d in ipairs(vim.diagnostic.get(0)) do print(d.lnum + 1, d.code, d.message) end" \
  -c "qa!"
```

```text
4 jscpd/duplicate-code Duplicated in src/holds.js:4-13 (80 tokens)
13 jscpd/similar-function Same structure as the function at src/holds.js:13-20 (1.00)
22 jscpd/complex-function Complexity 20 in loanStatus, over the limit of 15
```

With `src/index.js` and `src/format.js` in place of `src/loans.js`, the same command prints the dead code:

```text
3 unused-import `padLeft` is imported but never used (100%)
```

```text
7 unused-export exported function `formatFine` is never imported (85%)
```

The clone runs from line 4 to line 13 because it takes in the `export function` that opens the next function in both files. Edit `daysLate` in one of the files in the editor, and the clone diagnostic goes away before you save.

## The same findings from the command line

Each analysis of the server is a mode of the CLI:

```bash
jscpd .                   # Found 1 clones.
jscpd --similarity 0.85 . # Found 2 clones. (the second is similar (ast) ~1.00)
jscpd --dead-code .       # Found 2 dead code findings in 4 files (4.6% of 87 lines).
jscpd --complexity .      # src/loans.js first, at CX 27 for the whole file
```

The server counts complexity per function as well, which is why `loanStatus` gets a diagnostic of its own at CX 20.

## Semantic clones

The demo leaves the semantic analysis off, since it needs the embedding model. To try it, download the model once and add `"semantic": { "enabled": true }` to the `lsp` section:

```bash
jscpd --semantic-download
```

The first run embeds every function in the background, and the editor shows its progress.
