# jscpd-sarif-reporter

## 4.2.5

### Patch Changes

- Bug fixes: JSON reporter duplicate token counts, gitignore parent-directory walk, and Commander v15 migration

## 4.2.4

### Patch Changes

- Features
  - detectClonesAndStatistic() API — new function returning both clone results and statistics in one call; also exposes an optional statisticProvider injection point on detectClones. Closes #536, #549.
    Bug Fixes
  - .gitignore not respected by default — gitignore option now defaults to true; patterns are read from every scanned directory (not just process.cwd()); fix applies to both CLI and programmatic API. Use --no-gitignore to opt out. Fixes #790.
  - .gitignore negation patterns silently dropped — negated patterns (!test.js, !src/\*\*, etc.) were discarded instead of being passed to fast-glob. Fixes #723.
    Documentation
  - Document path option in .jscpd.json and package.json config examples. (#717)
  - Add Gitignore option section to README with CLI examples, config snippet, and default/type reference.
  - Add detectClonesAndStatistic API example to README.

## 4.2.3

### Patch Changes

- fix(finder): resolve relative ignore patterns against scan dirs (#611)

## 4.2.2

### Patch Changes

- fix(tokenizer): resolve quadratic bash tokenization hang

## 4.2.1

### Patch Changes

- fix tokenization issue for cross formats detection

## 4.2.0 — 2026-05-14

### Dependency Updates

- `@jscpd/core` → 4.2.0
- `@jscpd/finder` → 4.2.0

---

## 4.1.1

### Patch Changes

- Update hash function, improve performance and keep browser support

Outputs jscpd clone results in [SARIF](https://sarifweb.azurewebsites.net/) (Static Analysis Results Interchange Format). SARIF is the standard format understood by GitHub Code Scanning, Azure DevOps, Visual Studio, and many other developer tools — making it straightforward to surface copy-paste duplication as first-class code-scanning alerts in your pull requests.

---

## [4.1.0](https://www.npmjs.com/package/jscpd-sarif-reporter/v/4.1.0) — 2026-05-09

### Changes

- Aligned with the monorepo 4.1.0 release. No SARIF-specific changes.
- CI now tests against Node.js 22.x and 24.x.

---

## [4.0.7](https://www.npmjs.com/package/jscpd-sarif-reporter/v/4.0.7) — 2026-04-10

### Changes

- Aligned with the AI reporter release cycle.

---

## [4.0.6](https://www.npmjs.com/package/jscpd-sarif-reporter/v/4.0.6) — 2026-01-30

### Changes

- Aligned with the MCP server and GDScript support release.

---

## [4.0.5](https://www.npmjs.com/package/jscpd-sarif-reporter/v/4.0.5) — 2026-01-11

### Bug Fixes

- Fixed a build output issue.

---

## [4.0.4](https://www.npmjs.com/package/jscpd-sarif-reporter/v/4.0.4) — 2026-01-11

### Changes

- Merged community PRs; minor housekeeping.

---

## [4.0.3](https://www.npmjs.com/package/jscpd-sarif-reporter/v/4.0.3) — 2024-05-28

### Bug Fixes

- Fixed a missing `colors` runtime dependency that caused a crash on startup.
- Fixed package resolution issues on initial install.

---

## [4.0.0](https://www.npmjs.com/package/jscpd-sarif-reporter/v/4.0.0) — 2024-05-26

### New Package

First release of `jscpd-sarif-reporter`. Produces SARIF-formatted output from jscpd clone detection results.

**To use:**

```sh
jscpd --reporters sarif --output ./report
```

This generates a `jscpd-report.sarif` file you can upload to GitHub Code Scanning via the `github/codeql-action/upload-sarif` action.
