# CI & Pre-Commit Hooks

jscpd can enforce duplication thresholds in CI pipelines and as a local pre-commit hook — catching copy/pasted code before it reaches the main branch.

## GitHub Action

The [jscpd-copy-paste-detector](https://github.com/marketplace/actions/jscpd-copy-paste-detector) GitHub Action runs jscpd in your CI workflow. It installs the Rust engine, runs detection, uploads SARIF to GitHub Code Scanning, and optionally uploads the report as an artifact.

### Basic Usage

```yaml
name: Duplication Check

on: [push, pull_request]

jobs:
  jscpd:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: kucherenko/jscpd@master
```

This scans the entire repository with default settings and uploads SARIF results to GitHub Code Scanning.

### Fail on Threshold

Set `threshold` to fail the build when duplication exceeds a percentage:

```yaml
- uses: kucherenko/jscpd@master
  with:
    threshold: 5
```

The workflow fails if more than 5% of the code is duplicated.

### Action Inputs

| Input | Description | Default |
|-------|-------------|---------|
| `path` | Paths to scan (space-separated) | `.` |
| `config` | Path to `.jscpd.json` config file | — |
| `min-tokens` | Minimum tokens for a clone | `50` |
| `min-lines` | Minimum lines for a clone | `5` |
| `max-lines` | Maximum lines per block | — |
| `mode` | Detection mode: `mild`, `weak`, `strict` | `mild` |
| `format` | Comma-separated formats to check | — |
| `ignore` | Comma-separated glob patterns to ignore | — |
| `ignore-pattern` | Comma-separated regex patterns to skip | — |
| `reporters` | Comma-separated reporters | `console` |
| `output` | Output directory for file reporters | `report` |
| `threshold` | Max duplication % before exit 1 | — |
| `blame` | Enrich clones with git blame data | `false` |
| `exit-code` | Exit with code when duplicates found (`true` or integer) | — |
| `pattern` | Glob pattern for file search | — |
| `max-size` | Skip files larger than SIZE | — |
| `skip-local` | Skip clones in same directory | `false` |
| `ignore-case` | Ignore case of symbols (experimental) | `false` |
| `follow-symlinks` | Follow symbolic links | `false` |
| `no-gitignore` | Don't respect .gitignore files | `false` |
| `absolute` | Use absolute paths in reports | `false` |
| `formats-exts` | Custom format-to-extension mappings | — |
| `formats-names` | Custom format-to-filename mappings | — |
| `version` | jscpd version to install | `latest` |
| `install-prefix` | Installation directory for the binary | — |
| `skip-install` | Skip installation (binary already present) | `false` |
| `extra-args` | Additional arguments passed to jscpd | — |
| `upload-report` | Upload report directory as artifact | `false` |
| `upload-sarif` | Upload SARIF to GitHub Code Scanning | `true` |

### Action Outputs

| Output | Description |
|--------|-------------|
| `duplication-percentage` | Percentage of duplicated code found |
| `clones-found` | Number of clone pairs found |
| `duplicated-lines` | Number of duplicated lines |
| `total-lines` | Total lines scanned |
| `files-count` | Number of source files scanned |
| `report-path` | Path to the output directory |
| `sarif-path` | Path to the SARIF report file |
| `exit-code` | Exit code from jscpd |

### Examples

#### Scan specific directories with threshold

```yaml
- uses: kucherenko/jscpd@master
  with:
    path: src/lib src/utils
    threshold: 3
    ignore: "**/*.test.*,**/*.spec.*"
```

#### Use a config file

```yaml
- uses: kucherenko/jscpd@master
  with:
    config: .jscpd.json
    upload-report: true
```

#### Multi-reporter with artifact upload

```yaml
- uses: kucherenko/jscpd@master
  with:
    reporters: console,json,html,sarif
    output: jscpd-report
    upload-report: true
```

#### Pin a specific version

```yaml
- uses: kucherenko/jscpd@master
  with:
    version: "5.0.9"
```

#### Skip install (binary already in image)

```yaml
- uses: kucherenko/jscpd@master
  with:
    skip-install: true
```

#### Use outputs in subsequent steps

```yaml
- uses: kucherenko/jscpd@master
  id: jscpd

- name: Check results
  if: steps.jscpd.outputs.duplication-percentage > 5
  run: |
    echo "Duplication is ${{ steps.jscpd.outputs.duplication-percentage }}%"
    echo "Found ${{ steps.jscpd.outputs.clones-found }} clones"
```

## Pre-Commit Hook

Run jscpd before every commit to prevent duplicated code from entering the repository.

### Using pre-commit framework

The [pre-commit](https://pre-commit.com) framework manages git hooks for you. After configuring the hook, it runs automatically on every `git commit`.

**1. Install pre-commit** (one time, any of these):

```bash
# pip
pip install pre-commit

# brew
brew install pre-commit

# npm (wrapper around the Python tool)
npm install -g pre-commit
```

**2. Add the hook config** to `.pre-commit-config.yaml` in your repo:

**Option A: `language: node`** — pre-commit installs jscpd automatically:

```yaml
repos:
  - repo: local
    hooks:
      - id: jscpd
        name: jscpd - copy/paste detector
        entry: jscpd
        language: node
        additional_dependencies: ['jscpd@5']
        args: [--threshold, "5", --reporters, console,silent]
        pass_filenames: false
        always_run: true
```

**Option B: `language: system`** — jscpd must be pre-installed globally:

```yaml
repos:
  - repo: local
    hooks:
      - id: jscpd
        name: jscpd - copy/paste detector
        entry: jscpd
        language: system
        args: [--threshold, "5", --reporters, console,silent]
        pass_filenames: false
        always_run: true
```

If using Option B, install jscpd globally first: `npm install -g jscpd@5` or `cargo install jscpd`.

**3. Install the hook into git:**

```bash
pre-commit install
```

That's it — jscpd now runs on every `git commit`. If duplication exceeds the threshold, the commit is blocked.

To run manually without committing:

```bash
pre-commit run jscpd --all-files
```

### Using Husky

```bash
npm install -D husky
npx husky init
```

Add the hook:

```bash
echo 'npx jscpd@5 --threshold 5 --reporters console,silent .' > .husky/pre-commit
```

### Manual git hook

No extra tools required — just a shell script in `.git/hooks/`.

1. Create `.git/hooks/pre-commit`:

```bash
#!/bin/sh
jscpd --threshold 5 --reporters console,silent .
```

2. Make it executable:

```bash
chmod +x .git/hooks/pre-commit
```

Hooks in `.git/hooks/` are not version-controlled. To share the hook with your team, store it in the repo and symlink or copy it:

**Option A: Symlink from a versioned script**

Store the hook logic in the repo (e.g. `scripts/pre-commit`), then symlink:

```bash
ln -s ../../scripts/pre-commit .git/hooks/pre-commit
```

Each developer runs the symlink command once after cloning.

**Option B: `core.hooksPath` (Git 2.9+)**

Point Git at a versioned hooks directory:

```bash
git config core.hooksPath .githooks
```

Create `.githooks/pre-commit`:

```bash
#!/bin/sh
jscpd --threshold 5 --reporters console,silent .
```

```bash
chmod +x .githooks/pre-commit
```

Commit `.githooks/` to the repo. New contributors run the `git config` command once after cloning. Add it to your onboarding docs or a `scripts/setup.sh`:

```bash
#!/bin/sh
git config core.hooksPath .githooks
```

**Option C: npm `prepare` script**

Add to `package.json`:

```json
{
  "scripts": {
    "prepare": "git config core.hooksPath .githooks"
  }
}
```

`npm install` (and `npm ci`) automatically run `prepare`, so the hooks path is set with no manual steps.

**Option D: Makefile**

```makefile
.PHONY: hooks
hooks:
	git config core.hooksPath .githooks
```

Contributors run `make hooks` after cloning.

### Tips

- Use `--reporters console,silent` to show clone details without writing report files on every commit
- Use `--threshold` to set a failure threshold — the hook exits with code 1 if exceeded
- Use `--ignore` to exclude generated files, test fixtures, or vendor directories
- For large repos, use the Rust engine (`jscpd@5` / `cpd`) — it runs 24-37x faster, keeping commit latency low
- Consider `--format` to limit detection to specific languages during the hook, with a full scan in CI