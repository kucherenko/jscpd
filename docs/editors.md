# Editors

`jscpd --lsp` runs jscpd as a language server on stdin and stdout. An editor starts it for a workspace, and the server reports what jscpd finds as diagnostics in the files you edit. The diagnostics follow the text in the editor, saved or not: after you stop typing for 300 ms, the server tokenizes the file again from the buffer and searches its pool again. Files that change outside the editor, such as in a `git checkout`, reach the server when the editor watches files for it; the server reads them again and searches each pool they touch once. A folder deleted or moved in one piece counts for the files under it, and files the scan skips (ignored by `.gitignore` or `.ignore`, or over `maxSize`) stay out, whether they appear on disk or open in the editor.

The server runs five analyses. Only clones are on unless you turn the others on:

| Analysis | Name in `--lsp-analyses` | What it reports | Rules |
|---|---|---|---|
| Clones | `clones` | exact, renamed and near-miss copies | `jscpd/duplicate-code`, `jscpd/renamed-code`, `jscpd/similar-code` |
| Similar functions | `ast` | functions, classes, variables and type aliases whose syntax trees have the same shape, as `--similarity` finds them | `jscpd/similar-function` |
| Semantic clones | `semantic` | functions that do the same job, as `--semantic` finds them | `jscpd/semantic-code` |
| Dead code | `dead-code` | unused files, exports, symbols, imports and members, as `--dead-code` finds them | `unused-file`, `unused-export`, `unused-symbol`, `unused-import`, `unused-member` |
| Complexity | `complexity` | functions and files above a complexity limit | `jscpd/complex-function`, `jscpd/complex-file` |

Every diagnostic has the source `jscpd`, and its code is the id of its rule, which an editor can filter by. For clones and dead code, the SARIF reporters write the same ids, so a finding has one name in the editor and in code scanning.

See [`fixtures/lsp-demo`](../fixtures/lsp-demo/README.md) for a project that shows all of them but semantic clones.

## Setup

The editor needs `jscpd` on its `PATH` (see [Installation](rust.md#installation)). In each setup below, jscpd runs next to the language's own server rather than in place of it.

### Neovim

Neovim 0.11 and later:

```lua
vim.lsp.config('jscpd', {
  cmd = { 'jscpd', '--lsp' },
  root_markers = { '.jscpd.json', '.git' },
})
vim.lsp.enable('jscpd')
```

Without `filetypes`, Neovim starts the server for every file you open. To keep it to some languages, add them: `filetypes = { 'javascript', 'typescript', 'python' }`. The editor's settings go in `init_options`, for example `init_options = { lsp = { deadCode = { enabled = true } } }`.

Neovim 0.10:

```lua
vim.api.nvim_create_autocmd('FileType', {
  pattern = { 'javascript', 'typescript', 'python' },
  callback = function(args)
    vim.lsp.start({
      name = 'jscpd',
      cmd = { 'jscpd', '--lsp' },
      root_dir = vim.fs.root(args.buf, { '.jscpd.json', '.git' }),
    })
  end,
})
```

### Helix

In `languages.toml`:

```toml
[language-server.jscpd]
command = "jscpd"
args = ["--lsp"]

[[language]]
name = "typescript"
language-servers = ["typescript-language-server", "jscpd"]
```

A language gets the servers its `language-servers` list names, so the list has to keep the language's own server. Add a `[[language]]` table for every language you want jscpd in. The editor's settings go in the `config` key of the server: `config = { lsp = { deadCode = { enabled = true } } }`.

### Sublime Text

With the [LSP](https://packagecontrol.io/packages/LSP) package, in Preferences > Package Settings > LSP > Settings:

```json
{
  "clients": {
    "jscpd": {
      "enabled": true,
      "command": ["jscpd", "--lsp"],
      "selector": "source.js | source.ts | source.python"
    }
  }
}
```

The editor's settings go in `initialization_options`.

### Emacs

lsp-mode runs a client marked as an add-on next to the language's own server:

```elisp
(with-eval-after-load 'lsp-mode
  (lsp-register-client
   (make-lsp-client
    :new-connection (lsp-stdio-connection '("jscpd" "--lsp"))
    :activation-fn (lsp-activate-on "javascript" "typescript" "python")
    :add-on? t
    :server-id 'jscpd)))
```

Eglot connects one server to a buffer, so there jscpd would take the place of the language's own server.

### JetBrains IDEs

The [LSP4IJ](https://github.com/redhat-developer/lsp4ij) plugin starts a language server you define. In Settings > Languages & Frameworks > Language Servers, add a server with the command `jscpd --lsp` and map it to the file types you want.

### VS Code and Zed

Both start language servers only from extensions. Clients for them will live in a repository of their own.

## Projects

The `.jscpd.json` files split the workspace into projects, and the server looks for clones within a project, never across two:

- With no `.jscpd.json` in the workspace, the workspace is one project, whatever the number of its folders, and a clone between two folders counts.
- Each `.jscpd.json` makes its folder a project with that config. A config in a subfolder of another project splits that subfolder out, so a file belongs to the project of the nearest config above it.
- The files under no config form one more project, with the defaults.

When it starts, the server looks for `.jscpd.json` files, skipping `.git`, `node_modules` and what git ignores. It scans the workspace again when a config is saved in the editor, and when the editor reports that a config, a `.gitignore` or an `.ignore` file appeared, changed or went away. Only `.jscpd.json` makes a project: the server does not read `.config/jscpd.json` or the `jscpd` key of `package.json`, which the CLI also reads.

A `.jscpd.json` that is not valid JSON leaves its project on the defaults, and the editor shows the parse error. A key in the `semantic` section that looks like a secret, such as `apiKey`, stops the project: the editor shows why, and the project gets no diagnostics until the key is gone. Other warnings about a config, such as an unknown key, go to the editor's log.

## Options

Options come from three places, and each wins over the one before it:

1. The command line that starts the server. Detection options such as `--min-tokens`, `--format` or `--ignore`, and `--lsp-analyses`, are the defaults for every project.
2. The project's `.jscpd.json`.
3. The editor's settings, which the editor sends when it starts the server (`initializationOptions`) and when they change (`workspace/didChangeConfiguration`). They take the keys of `.jscpd.json`, at the top level or under a `jscpd` key, and apply to every project on top of its config.

A change to a config file or to the editor's settings applies at once, without a restart. jscpd refuses `--semantic`, `--dead-code` and `--complexity` together with `--lsp`; turn those analyses on with `--lsp-analyses` or in the `lsp` section instead. It refuses `--config` and paths too: each project reads its own `.jscpd.json`, and the folders come from the editor.

A mistake in the `lsp` settings of the editor, such as an unknown key, is shown to the user, and those settings are left out while the `lsp` section of each config still applies.

### The `lsp` section

```json
{
  "lsp": {
    "clones": { "enabled": true, "warningTokens": 100 },
    "ast": { "enabled": true, "similarity": 0.85 },
    "semantic": { "enabled": false },
    "deadCode": { "enabled": true },
    "complexity": { "enabled": true, "functionLimit": 15 },
    "allFiles": false
  }
}
```

| Key | What it does | Default |
|---|---|---|
| `<analysis>.enabled` | turns the analysis on or off, over `--lsp-analyses` | on for `clones`, off for the rest |
| `clones.warningTokens` | clones of at least this many tokens are warnings, and smaller ones information | every clone is a warning |
| `ast.similarity` | how much of their syntax-tree shape two functions must share, from 0 to 1 | the `similarity` key of the config when it is below 1, else 0.85 |
| `complexity.functionLimit` | a function above this complexity gets a diagnostic | 15 |
| `allFiles` | also publish the diagnostics of closed files that have clones, semantic pairs or dead code, so a problems panel lists the whole project | `false` |

A closed file whose only finding is its complexity stays out of `allFiles`: listing it would mean parsing every file of the project after each change.

The other options of each analysis stay where the CLI reads them: detection options at the top level of `.jscpd.json`, the model and thresholds in its `semantic` section, and entry points, categories and the confidence floor in its `deadCode` section. The `enabled` key of the top-level `deadCode` section picks the CLI's mode and does not turn the analysis on in the server; the one in `lsp.deadCode` does.

## Analyses

### Clones

Each fragment of a clone in an open file gets a warning over its range, as in SARIF, and a clone within one file gets one on each of its ranges. The message names the other copy, such as `Duplicated in src/holds.js:4-13 (80 tokens)`, or all of them when a block has several: `Duplicated in 3 places: ...`. In a project with several folders, a path starts with the name of its folder. Editors that support `relatedInformation` also list each copy as a link.

Renamed and near-miss copies need the options that find them on the command line: `ignoreIdentifiers`, `ignoreLiterals` or `ignoreAnnotations` for renamed code, and `maxGapLines` for copies with a few changed lines between them.

Two code actions come with a clone:

- "Go to the other copy in src/holds.js:4-13" opens that copy, with one action per copy.
- "Ignore this clone" puts `jscpd:ignore-start` and `jscpd:ignore-end` comments around the fragment, in the comment syntax of the file's language: on lines of their own where the fragment starts and ends its lines, and as block comments beside it where other code shares a line with it. The first line of a file that starts with `#!` or `<?` stays first.

A hover over a clone shows the message and the first lines of the other copy.

### Similar functions

The server runs the pass of `--similarity` on JavaScript, TypeScript, JSX, TSX and Python, and on their code blocks in Markdown and components. It summarizes the syntax tree of every function, class, variable and type alias, with the called methods when the config sets `similarityIdentifiers` to `role-aware`, with its literals as `similarityLiterals` says and its Python decorators as `similarityDecorators` does, among the units `similarityCandidates` and `similaritySkipTests` keep, and two functions pair when their summaries share at least the ratio `ast.similarity` asks for. A pair gets one diagnostic of severity information on the first line of each function: `Same structure as the function at src/holds.js:13-20 (1.00)`, or `the class at` for a pair of classes. A pair that a clone already covers is not reported again. "Go to the similar function" (or class, variable, type alias) and the hover work as they do for clones.

### Semantic clones

Semantic clones come from the pass of `--semantic`, with its languages and thresholds. The server runs it in the background when it starts and after each save, and the editor shows its progress. The first run embeds every function; the vectors go to the cache on disk that `--semantic` keeps, so later runs embed only the functions that changed. A pair gets one diagnostic of severity information on the first line of each function: `Does the same job as the function at src/money.ts:3-9 (0.87)`, with "Go to the similar function" and a hover as for clones. The pairs come from the files on disk, so while either file differs from what the last run read, as after an unsaved edit, its pairs are hidden until the next run.

The server never downloads the model on its own. When the model is missing, it asks first, and it shows the download as progress. `jscpd --semantic-download` downloads it from the command line instead.

A `semantic.url` in `.jscpd.json` or in the editor's settings is used only when it points to this machine, since a config can arrive with the code. To use a remote embeddings API, give it with `--semantic-url` in the command that starts the server, and the key in `JSCPD_SEMANTIC_API_KEY`.

### Dead code

The engine behind `--dead-code`, basta, finds it in JavaScript, TypeScript, JSX, TSX, Vue, Svelte, Astro and Python. The import graph spans the project, so the server runs basta in the background when it starts and after each save, on the files on disk, and the editor shows its progress. A finding at or above the confidence floor (`minConfidence`, 60 by default) gets a diagnostic of severity hint with the `Unnecessary` tag, so editors fade the code instead of underlining it. The message carries the confidence, such as ``exported function `formatFine` is never imported (85%)``, and the hover lists the reasons behind a score below 100. An unused file gets one diagnostic on its first line. The findings come from the files on disk, so while a file differs from what the last run read, as after an unsaved edit, its findings are hidden until the run after the next save.

Mistakes in the `deadCode` section, such as an unknown category, are shown to the user, and dead code stays off for that project. The server leaves Rust to rust-analyzer, so `deadCode.rustDiagnostics` has no effect here. The server offers no code action that deletes code.

### Complexity

The measure is the cyclomatic estimate of `--summary` and `--complexity`: one path, plus one for every branch token (`if`, `for`, `while`, `case`, `catch`, `&&`, `||`, a ternary `?` and the rest). The server counts it per function too, over the range of each function it finds in JavaScript, TypeScript, JSX, TSX, Vue, Svelte, Astro, Python, Rust, Go, Java, Kotlin, C#, C, C++, PHP, Ruby, Scala and Swift. It recounts a file from the buffer after each edit.

- A function above `complexity.functionLimit` gets a `jscpd/complex-function` diagnostic of severity information on its first line: `Complexity 20 in loanStatus, over the limit of 15`.
- A file at or above the complex-file bar of the health score (`health.complexFile`, 50 by default) gets a `jscpd/complex-file` diagnostic on its first line.

## Requests for editor clients

Clients with views of their own, such as a tree of clones, need data that diagnostics do not carry. The server answers these requests. Each answer holds one entry per project in `projects`, with its `roots` and its `config` file:

| Request | Each project's entry |
|---|---|
| `jscpd/clones` | `statistics` and `duplicates`, as in `jscpd-report.json` |
| `jscpd/semantic` | `duplicates` of the semantic pairs, for projects with the analysis on |
| `jscpd/deadCode` | `findings`, as in `basta-report.json` but with absolute paths, for projects with the analysis on |
| `jscpd/complexity` | `summary`, as in `jscpd-complexity.json` with `--absolute`, with the duplication columns filled in |
| `jscpd/statistics` | the number of `files` and the `statistics` |

`jscpd/rescan` scans every project again and answers `null`. The command `jscpd.showLocation`, with a URI and a range as its arguments, is the one behind "Go to the other copy". The server answers it with `window/showDocument`.

## Positions and logs

The progress of a scan and of each background run ends with what it found, such as `94 files, 13 clones` or `4 found`, so an editor that shows progress also shows that the server runs in a project with nothing to report in the open file.

Lines count from 0, and columns count UTF-16 code units unless the editor offers UTF-8. stdout carries protocol messages only. The server shows the user what changes what runs, such as a config that does not parse or a wrong `lsp` or `deadCode` section, and writes other warnings to the editor's log; it writes to stderr only when it cannot start or loses the connection.

## Limits

- Semantic clones and dead code follow saves, not keystrokes.
- The server pushes diagnostics and does not answer `textDocument/diagnostic` requests.
- An edit searches the whole detection pool of its file again, on the thread that answers the editor. In a pool of about 12,700 JavaScript files that takes several seconds; looking up only the changed file (#1001) would fix that.
- With `followSymlinks`, files reached through a symlinked folder are scanned but get no diagnostics of their own.
