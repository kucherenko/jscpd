# Examples

| Directory | Engine | What it shows |
|-----------|--------|---------------|
| [`api/`](api) | **v4 (TypeScript)** | The Node.js programming API: `jscpd()` and `detectClones()` from the `jscpd` / `@jscpd/core` packages, a LevelDB persistent store, and a GitHub workflow that runs jscpd and comments on pull requests. The v5 Rust engine has no Node.js API; use `jscpd@4` for these. |
| [`rust-cpd-finder/`](rust-cpd-finder) | **v5 (Rust)** | Embedding the Rust engine through the [`cpd-finder`](https://crates.io/crates/cpd-finder) crate: configure a run, execute it, and read the clones and statistics. |

For the CLI (both engines), the GitHub Action, Docker image, and pre-commit hooks see the [documentation](https://jscpd.dev) and [`docs/`](../docs).
