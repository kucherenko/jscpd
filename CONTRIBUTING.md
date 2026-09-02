# Contributing to jscpd v4

Thanks for considering a contribution! This document describes how to propose
changes and what is required for a contribution to be accepted.

> This branch (`master-v4`) is the maintenance branch for **jscpd v4**, the
> TypeScript engine. It receives security and critical fixes. New features land
> in the current major version, v5, on
> [`master`](https://github.com/kucherenko/jscpd) — please open feature
> requests and v5 bug reports there.

## How to contribute

1. Fork [kucherenko/jscpd](https://github.com/kucherenko/jscpd/) and clone your fork.
2. Create a feature branch from `master-v4`.
3. Make your changes (see the workflow below).
4. Open a pull request **against `master-v4`** describing what the change does
   and why. Pull requests targeting `master` are for the v5 engine.

Bug reports go through
[GitHub Issues](https://github.com/kucherenko/jscpd/issues) — mention that you
are running v4 (`jscpd --version` prints `4.x.y`). Security vulnerabilities
must **not** be reported publicly — see [SECURITY.md](SECURITY.md).

## Development setup

The repository is a pnpm workspace: the CLI and server live in [`apps/`](apps),
the libraries in [`packages/`](packages).

```bash
pnpm install                    # install dependencies (pnpm 10)
pnpm build                      # build every package (turbo)
pnpm test                       # test suite (vitest)
pnpm lint                       # ESLint
pnpm dev                        # watch mode
```

Run the built CLI against the fixtures corpus to try your change end-to-end:

```bash
node apps/jscpd/bin/jscpd ./fixtures --reporters console,json --output report
```

The API examples live in [`examples/api`](examples/api).

### Changesets

Releases are driven by [changesets](https://github.com/changesets/changesets).
Every user-visible change needs a changeset describing it:

```bash
pnpm changeset
```

Commit the generated file under `.changeset/` with your change. Merging the
"Version Packages" pull request bumps the package versions and the release
workflow publishes them to npm under the `latest-4` dist-tag.

## Requirements for acceptable contributions

- **Tests are required.** New functionality must come with tests that exercise
  it, and bug fixes should include a test that fails without the fix. Look at
  the existing tests next to the code you touch (`__tests__` directories) and
  follow their patterns.
- **CI must be green.** ESLint is enforced, and the CI smoke test runs the
  built CLI against `fixtures/`.
- **Match the surrounding style** — naming, comment density, and idioms of the
  file you are editing.
- **Keep changes focused.** One logical change per pull request; unrelated
  refactoring belongs in its own PR.
- By submitting a contribution you agree that it is licensed under the
  project's [MIT license](LICENSE).

## Code of conduct

All interactions are covered by the [Code of Conduct](CODE_OF_CONDUCT.md).
