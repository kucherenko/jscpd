#!/usr/bin/env python3
"""Repack the release tarballs into PyPI platform wheels.

The npm publish matrix already builds one `jscpd-<platform-key>.tar.gz` per
supported target (binary + LICENSE at the archive root). A wheel is a zip with
the same binary under `<name>-<version>.data/scripts/`, which pip/uv install
onto PATH — exactly what maturin's `bin` bindings emit — so there is nothing
to compile here. One wheel per tarball, one PyPI project (`jscpd`) exposing
both the `jscpd` and `cpd` commands, matching `cargo install jscpd`.

From the repository root (release.yml runs it from there and from rust/):

    python3 rust/scripts/build-pypi-wheels.py --version 5.2.0 \
        --assets-dir release-assets --out-dir dist

    python3 rust/scripts/build-pypi-wheels.py --pep440 5.3.0-beta.1   # -> 5.3.0b1

Only the standard library is used so the script runs on a bare CI runner.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import os
import re
import sys
import tarfile
import zipfile
from pathlib import Path

PROJECT = "jscpd"
SCRIPTS = ("jscpd", "cpd")

# Platform keys are the npm-style names used by the release assets
# (rust/npm/prebuilt-targets.json). glibc wheels take their manylinux floor
# from the binary itself (see `glibc_floor`), because it depends on the
# runner image the target was built on, not on anything we choose.
PLATFORMS = {
    "linux-x64-gnu": {"arch": "x86_64"},
    "linux-arm64-gnu": {"arch": "aarch64"},
    "linux-x64-musl": {"tag": "musllinux_1_2_x86_64"},
    "linux-arm64-musl": {"tag": "musllinux_1_2_aarch64"},
    # Rust's minimum supported macOS: 10.12 on x86_64, 11.0 on arm64.
    "darwin-x64": {"tag": "macosx_10_12_x86_64"},
    "darwin-arm64": {"tag": "macosx_11_0_arm64"},
    "windows-x64-msvc": {"tag": "win_amd64", "exe": ".exe"},
    "windows-arm64-msvc": {"tag": "win_arm64", "exe": ".exe"},
}

SUMMARY = "Copy/paste detector for programming source code"

LONG_DESCRIPTION = """\
# jscpd

Copy/paste detector for programming source code: a self-contained Rust binary
that finds duplicated blocks across 150+ formats and reports them as console
output, JSON, HTML, SARIF, Markdown and more.

This package ships the prebuilt `jscpd` and `cpd` commands (same binary) as
platform wheels — no Python code and no Node.js runtime involved.

```bash
pip install jscpd          # or: pipx install jscpd / uv tool install jscpd
jscpd /path/to/code

uvx jscpd /path/to/code    # run without installing
```

As a [pre-commit](https://pre-commit.com) hook:

```yaml
repos:
  - repo: local
    hooks:
      - id: jscpd
        name: jscpd - copy/paste detector
        entry: jscpd
        language: python
        additional_dependencies: ['jscpd==__VERSION__']
        args: [--threshold, "5", --reporters, console,silent]
        pass_filenames: false
        always_run: true
```

Documentation: https://jscpd.dev — Source: https://github.com/kucherenko/jscpd
"""

CLASSIFIERS = (
    "Development Status :: 5 - Production/Stable",
    "Environment :: Console",
    "Intended Audience :: Developers",
    "License :: OSI Approved :: MIT License",
    "Operating System :: MacOS",
    "Operating System :: Microsoft :: Windows",
    "Operating System :: POSIX :: Linux",
    "Programming Language :: Python :: 3",
    "Programming Language :: Rust",
    "Topic :: Software Development :: Quality Assurance",
)

_PRERELEASE = {"alpha": "a", "beta": "b", "rc": "rc"}

# Files inside a wheel get a fixed timestamp (zip cannot encode anything
# earlier than 1980) so rebuilding the same release yields identical bytes.
_EPOCH = (1980, 1, 1, 0, 0, 0)


def pep440(version: str) -> str:
    """Map the project's semver version to PEP 440.

    `5.3.0` stays as is; `5.3.0-beta.1` becomes `5.3.0b1` (also alpha/rc,
    with or without the dot before the number). Anything else is rejected
    rather than guessed, because a wrong mapping would publish a version
    PyPI orders differently from the semver one.
    """
    m = re.fullmatch(r"(\d+\.\d+\.\d+)(?:-(alpha|beta|rc)\.?(\d+))?", version)
    if not m:
        raise SystemExit(f"cannot map version {version!r} to PEP 440")
    base, kind, num = m.groups()
    return base if kind is None else f"{base}{_PRERELEASE[kind]}{num}"


def glibc_floor(binary: bytes) -> tuple[int, int]:
    """Highest `GLIBC_x.y` symbol version the binary references."""
    versions = {(int(a), int(b)) for a, b in re.findall(rb"GLIBC_(\d+)\.(\d+)", binary)}
    if not versions:
        raise SystemExit("no GLIBC_* symbol versions found in a glibc binary")
    return max(versions)


def platform_tag(key: str, binary: bytes) -> str:
    spec = PLATFORMS[key]
    if "tag" in spec:
        return spec["tag"]
    major, minor = glibc_floor(binary)
    return f"manylinux_{major}_{minor}_{spec['arch']}"


def read_tarball(path: Path, exe: str) -> tuple[bytes, bytes]:
    """Return (binary, license) from a `jscpd-<key>.tar.gz` release asset."""
    with tarfile.open(path, "r:gz") as tar:
        members = {os.path.basename(m.name): m for m in tar.getmembers() if m.isfile()}
        binary_name = "jscpd" + exe
        for name in (binary_name, "LICENSE"):
            if name not in members:
                raise SystemExit(f"{path.name}: missing {name}")
        binary = tar.extractfile(members[binary_name]).read()
        license_text = tar.extractfile(members["LICENSE"]).read()
    return binary, license_text


def metadata(version: str) -> bytes:
    lines = [
        "Metadata-Version: 2.1",
        f"Name: {PROJECT}",
        f"Version: {version}",
        f"Summary: {SUMMARY}",
        "Author: Andrey Kucherenko",
        "License: MIT",
        "Keywords: duplicate-code,clone-detection,copy-paste,cpd,static-analysis",
        "Project-URL: Homepage, https://jscpd.dev",
        "Project-URL: Documentation, https://jscpd.dev",
        "Project-URL: Repository, https://github.com/kucherenko/jscpd",
        "Project-URL: Changelog, https://jscpd.dev/getting-started/changelog",
        "Project-URL: Issues, https://github.com/kucherenko/jscpd/issues",
        "Requires-Python: >=3.8",
        "Description-Content-Type: text/markdown",
    ]
    lines.extend(f"Classifier: {c}" for c in CLASSIFIERS)
    body = LONG_DESCRIPTION.replace("__VERSION__", version)
    return ("\n".join(lines) + "\n\n" + body).encode()


class WheelWriter:
    """Minimal wheel writer: fixed timestamps, unix modes, RECORD at the end."""

    def __init__(self, path: Path):
        self._zip = zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED)
        self._record: list[str] = []

    def add(self, arcname: str, data: bytes, mode: int = 0o644) -> None:
        info = zipfile.ZipInfo(arcname, date_time=_EPOCH)
        info.compress_type = zipfile.ZIP_DEFLATED
        info.external_attr = (0o100000 | mode) << 16  # regular file + mode
        self._zip.writestr(info, data)
        digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=")
        self._record.append(f"{arcname},sha256={digest.decode()},{len(data)}")

    def close(self, record_name: str) -> None:
        self._record.append(f"{record_name},,")
        info = zipfile.ZipInfo(record_name, date_time=_EPOCH)
        info.compress_type = zipfile.ZIP_DEFLATED
        info.external_attr = (0o100000 | 0o644) << 16
        self._zip.writestr(info, "\n".join(self._record) + "\n")
        self._zip.close()


def build_wheel(key: str, tarball: Path, version: str, out_dir: Path) -> Path:
    spec = PLATFORMS[key]
    exe = spec.get("exe", "")
    binary, license_text = read_tarball(tarball, exe)
    tag = f"py3-none-{platform_tag(key, binary)}"

    dist_info = f"{PROJECT}-{version}.dist-info"
    scripts_dir = f"{PROJECT}-{version}.data/scripts"
    wheel_path = out_dir / f"{PROJECT}-{version}-{tag}.whl"

    w = WheelWriter(wheel_path)
    for script in SCRIPTS:
        w.add(f"{scripts_dir}/{script}{exe}", binary, 0o755)
    w.add(f"{dist_info}/METADATA", metadata(version))
    w.add(
        f"{dist_info}/WHEEL",
        f"Wheel-Version: 1.0\nGenerator: jscpd-release\nRoot-Is-Purelib: false\nTag: {tag}\n".encode(),
    )
    w.add(f"{dist_info}/LICENSE", license_text)
    w.close(f"{dist_info}/RECORD")
    return wheel_path


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", help="release version as in rust/package.json")
    parser.add_argument(
        "--assets-dir", type=Path, help="directory holding jscpd-<key>.tar.gz"
    )
    parser.add_argument("--out-dir", type=Path, help="where to write the wheels")
    parser.add_argument(
        "--pep440", metavar="VERSION", help="print the PEP 440 form of VERSION and exit"
    )
    args = parser.parse_args(argv)

    if args.pep440:
        print(pep440(args.pep440))
        return 0
    if not (args.version and args.assets_dir and args.out_dir):
        parser.error("--version, --assets-dir and --out-dir are required")

    version = pep440(args.version)
    args.out_dir.mkdir(parents=True, exist_ok=True)

    missing = [
        k for k in PLATFORMS if not (args.assets_dir / f"jscpd-{k}.tar.gz").is_file()
    ]
    if missing:
        raise SystemExit(
            f"missing release assets in {args.assets_dir}: {', '.join(missing)}"
        )

    for key in PLATFORMS:
        wheel = build_wheel(
            key, args.assets_dir / f"jscpd-{key}.tar.gz", version, args.out_dir
        )
        print(f"{key:<20} -> {wheel.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
