# Contributing to jscpd

Thanks for considering a contribution! This document describes how to propose
changes and what is required for a contribution to be accepted.

## How to contribute

1. Fork [kucherenko/jscpd](https://github.com/kucherenko/jscpd/) and clone your fork.
2. Create a feature branch from `master`.
3. Make your changes (see the workflow below).
4. Open a pull request against `master` describing what the change does and why.

Bug reports and feature requests go through
[GitHub Issues](https://github.com/kucherenko/jscpd/issues). Security
vulnerabilities must **not** be reported publicly — see [SECURITY.md](SECURITY.md).

## Development setup

`master` contains the Rust engine (v5) in [`rust/`](rust): the `jscpd` / `cpd`
binary, its four library crates, and the npm wrapper packages.

```bash
cd rust
cargo build
cargo nextest run --workspace   # full test suite (cargo install cargo-nextest)
cargo test --workspace          # equivalent without nextest
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all                 # formatting is enforced
```

End-to-end check against the multi-format corpus in [`fixtures/`](fixtures)
(the same run CI performs in the `smoke` job):

```bash
cd rust
cargo build --release -p jscpd
./target/release/jscpd ../fixtures --reporters console,json --output ../smoke-report --min-tokens 50
```

A Rust API example lives in [`examples/rust-cpd-finder`](examples/rust-cpd-finder).
After changing the tokenizer's format table, regenerate
[`FORMATS.md`](FORMATS.md) with `node rust/scripts/gen-formats-md.mjs`.

The Rust test suite is not run in PR CI — run it locally before submitting.

### jscpd v4 (TypeScript)

The TypeScript engine is maintained on the
[`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4) branch and
receives security and critical fixes only. Open v4 pull requests against
`master-v4`; its own `CONTRIBUTING.md` describes the pnpm-based workflow.

## Requirements for acceptable contributions

- **Tests are required.** New functionality must come with tests that exercise
  it, and bug fixes should include a test that fails without the fix. As a
  rule of thumb, look at the existing tests next to the code you touch
  (`#[cfg(test)]` modules and `tests/` directories) and follow their patterns.
- **CI must be green.** Lints and formatting are enforced: `cargo clippy -D
  warnings` and `cargo fmt --check`.
- **Match the surrounding style** — naming, comment density, and idioms of the
  file you are editing.
- **Keep changes focused.** One logical change per pull request; unrelated
  refactoring belongs in its own PR.
- By submitting a contribution you agree that it is licensed under the
  project's [MIT license](LICENSE).

## Code of conduct

All interactions are covered by the [Code of Conduct](CODE_OF_CONDUCT.md).
