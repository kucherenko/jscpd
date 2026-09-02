# Contributing to jscpd

Thanks for considering a contribution! This document describes how to propose
changes and what is required for a contribution to be accepted.

## How to contribute

1. Fork [kucherenko/jscpd](https://github.com/kucherenko/jscpd/) and clone your fork.
2. Create a feature branch from `master`.
3. Make your changes (see the workflows below).
4. Open a pull request against `master` describing what the change does and why.

Bug reports and feature requests go through
[GitHub Issues](https://github.com/kucherenko/jscpd/issues). Security
vulnerabilities must **not** be reported publicly — see [SECURITY.md](SECURITY.md).

## Development setup

The repository contains two engines:

- **Rust engine (v5, active development)** in [`rust/`](rust) — the `cpd` binary
  and its crates.
- **TypeScript packages (v4, maintenance)** in [`packages/`](packages) and
  [`apps/`](apps) — security and critical fixes only.

### Rust engine

```bash
cd rust
cargo build
cargo nextest run --workspace   # full test suite (cargo install cargo-nextest)
cargo test --workspace          # equivalent without nextest
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all                 # formatting is enforced
```

Rust API examples live in [`examples/rust-cpd-finder`](examples/rust-cpd-finder); the
TypeScript API examples in [`examples/api`](examples/api) target v4 only.

The Rust test suite is not run in PR CI — run it locally before submitting.

### TypeScript packages

```bash
pnpm install
pnpm dev                        # run in dev mode
pnpm test                       # test suite
pnpm build
```

## Requirements for acceptable contributions

- **Tests are required.** New functionality must come with tests that exercise
  it, and bug fixes should include a test that fails without the fix. As a
  rule of thumb, look at the existing tests next to the code you touch
  (`#[cfg(test)]` modules and `tests/` directories in Rust, `__tests__` in
  TypeScript) and follow their patterns.
- **CI must be green.** Lints and formatting are enforced: `cargo clippy -D
  warnings` and `cargo fmt --check` for Rust, ESLint for TypeScript.
- **Match the surrounding style** — naming, comment density, and idioms of the
  file you are editing.
- **Keep changes focused.** One logical change per pull request; unrelated
  refactoring belongs in its own PR.
- By submitting a contribution you agree that it is licensed under the
  project's [MIT license](LICENSE).

## Code of conduct

All interactions are covered by the [Code of Conduct](CODE_OF_CONDUCT.md).
