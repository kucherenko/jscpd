# Security Policy

## Supported Versions

| Version | Supported | Branch |
| ------- | --------- | ------ |
| 5.x (current major) | :white_check_mark: Active development | [`master`](https://github.com/kucherenko/jscpd) |
| 4.x (TypeScript engine) | :white_check_mark: Security and critical fixes | `master-v4` (this branch) |
| < 4.0 | :x: | — |

Security fixes for the 4.x line are developed on `master-v4` and published to
npm as `jscpd@4` (`latest-4` dist-tag) together with the `@jscpd/*` packages.
Fixes for 5.x ship from `master`.

## Reporting a Vulnerability

Please report security vulnerabilities **privately** — do not open a public issue.

- Preferred: [GitHub private vulnerability reporting](https://github.com/kucherenko/jscpd/security/advisories/new)
- Alternatively: email kucherenko.andrey@gmail.com with "jscpd security" in the subject

Please include a description of the issue, steps to reproduce, affected versions, and any known impact.

You can expect an acknowledgement within 7 days. Once a fix is available, it is released for the supported version lines and the advisory is published with credit to the reporter (unless you prefer to stay anonymous).

Fixed vulnerabilities are identified in the release notes and changelog of the release that contains the fix, with a reference to the published advisory (past examples: the ReDoS fixes in [`CHANGELOG.md`](CHANGELOG.md)).
