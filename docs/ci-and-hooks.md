# CI & Pre-Commit Hooks

jscpd can enforce duplication thresholds in CI pipelines and as a local pre-commit hook — catching copy/pasted code before it reaches the main branch.

This page covers **jscpd v4** (the Node.js engine, `jscpd@4`). The `kucherenko/jscpd` GitHub Action (`uses: kucherenko/jscpd@v5`) and the `ghcr.io/kucherenko/jscpd` Docker image ship the v5 engine only; they are documented on [`master`](https://github.com/kucherenko/jscpd) and at https://jscpd.dev. With v4, run the CLI through `npx jscpd@4` as shown below.

## GitHub Actions

### Basic usage

```yaml
name: Duplication Check

on: [push, pull_request]

jobs:
  jscpd:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - name: Check for duplicated code
        run: npx jscpd@4 --threshold 5 --reporters console,silent .
```

The job fails when more than 5% of the code is duplicated. `jscpd@4` resolves to the newest 4.x release (`latest-4` dist-tag); pin an exact version (`npx jscpd@4.3.0`) for reproducible runs.

### Upload SARIF to GitHub Code Scanning

The `sarif` reporter writes `jscpd-sarif.json`, which GitHub's code-scanning upload accepts directly:

```yaml
jobs:
  jscpd:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      security-events: write
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - name: Run jscpd
        run: npx jscpd@4 --reporters console,sarif --output report .
      - name: Upload SARIF
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: report/jscpd-sarif.json
```

### Keep the reports as artifacts

```yaml
      - name: Run jscpd
        run: npx jscpd@4 --reporters console,json,html,markdown --output report .
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: jscpd-report
          path: report
```

### Use a config file

Commit a `.jscpd.json` next to your sources and run without flags:

```yaml
      - run: npx jscpd@4
```

```json
{
  "threshold": 5,
  "reporters": ["console", "json"],
  "ignore": ["**/node_modules/**", "**/dist/**", "**/*.min.js"],
  "gitignore": true
}
```

### Comment on pull requests

[`examples/api/example_github_action.yml`](../examples/api/example_github_action.yml) is a complete workflow that runs jscpd, filters the JSON report down to the files changed in the pull request, and posts the clones as a PR comment.

### Read the results in later steps

The JSON report (`--reporters json`) is easy to query with `jq`:

```yaml
      - run: npx jscpd@4 --reporters console,json --output report .
      - name: Fail on more than 10 clones
        run: |
          clones=$(jq '.statistics.total.clones' report/jscpd-report.json)
          echo "clones=$clones"
          [ "$clones" -le 10 ]
```

`statistics.total` also carries `percentage`, `duplicatedLines`, `lines`, and `sources`.

## Other CI systems

Anything that can run Node.js can run jscpd. GitLab CI:

```yaml
duplication:
  image: node:22
  script:
    - npx jscpd@4 --threshold 5 --reporters console,json --output report .
  artifacts:
    when: always
    paths:
      - report/
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
        additional_dependencies: ['jscpd@4']
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

If using Option B, install jscpd globally first: `npm install -g jscpd@4`.

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
echo 'npx jscpd@4 --threshold 5 --reporters console,silent .' > .husky/pre-commit
```

If jscpd is a devDependency of the project (`npm install -D jscpd@4`), `npx jscpd` uses that local copy.

### Manual git hook

No extra tools required — just a shell script in `.git/hooks/`.

1. Create `.git/hooks/pre-commit`:

```bash
#!/bin/sh
npx jscpd@4 --threshold 5 --reporters console,silent .
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
npx jscpd@4 --threshold 5 --reporters console,silent .
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
- Use `--noTips` to keep CI logs free of hint lines
- For large repositories, `--store leveldb` keeps the token map on disk instead of in memory
- Consider `--format` to limit detection to specific languages during the hook, with a full scan in CI
