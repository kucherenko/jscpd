# Security Policy

## Supported Versions

| Version | Branch | Supported |
| ------- | ------ | --------- |
| 5.x (Rust engine) | [`master`](https://github.com/kucherenko/jscpd) | :white_check_mark: Active development |
| 4.x (TypeScript engine) | [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4) | :white_check_mark: Security fixes only |
| < 4.0 | — | :x: |

## Reporting a Vulnerability

Please report security vulnerabilities **privately** — do not open a public issue.

- Preferred: [GitHub private vulnerability reporting](https://github.com/kucherenko/jscpd/security/advisories/new)
- Alternatively: email kucherenko.andrey@gmail.com with "jscpd security" in the subject

Please include a description of the issue, steps to reproduce, affected versions, and any known impact.

You can expect an acknowledgement within 7 days. Once a fix is available, it is released for the supported version lines and the advisory is published with credit to the reporter (unless you prefer to stay anonymous).

Fixed vulnerabilities are identified in the release notes and changelog of the release that contains the fix, with a reference to the published advisory: [`rust/CHANGELOG.md`](rust/CHANGELOG.md) for 5.x, and `CHANGELOG.md` on the `master-v4` branch for 4.x.
