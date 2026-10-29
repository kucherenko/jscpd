#!/usr/bin/env python3
"""Bundle Prism (https://prismjs.com, MIT) for the HTML report.

The HTML report highlights the code of a clone with Prism in the browser. jscpd
keeps the Prism core and every Prism language in one file,
crates/cpd-reporter/assets/prism/prism.min.js, and each report takes the core
and the languages of its clones from it (crates/cpd-reporter/src/prism.rs).

Each language in the bundle starts on a line of its own:

    /*prism-component <id> <require> <after>*/

`require` lists the languages it needs, `after` the ones that, when the page has
them too, load before it (Prism's `optional` and `modify`); `-` stands for none.

To update Prism, download the package from npm, check it against the integrity
npm publishes, unpack it, and run from the repository root:

    npm view prismjs@1.30.0 dist.integrity dist.tarball
    curl -sSfLO https://registry.npmjs.org/prismjs/-/prismjs-1.30.0.tgz
    echo "sha512-$(openssl dgst -sha512 -binary prismjs-1.30.0.tgz | openssl base64 -A)"
    tar -xzf prismjs-1.30.0.tgz
    python3 rust/scripts/vendor-prism.py package

Only the standard library is used.
"""

import json
import shutil
import sys
from pathlib import Path

ASSETS = Path(__file__).resolve().parent.parent / "crates/cpd-reporter/assets/prism"


def as_list(value):
    if value is None:
        return []
    return [value] if isinstance(value, str) else list(value)


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: vendor-prism.py <unpacked prismjs package dir>")
    package = Path(sys.argv[1])
    version = json.loads((package / "package.json").read_text(encoding="utf-8"))["version"]
    languages = json.loads((package / "components.json").read_text(encoding="utf-8"))["languages"]
    components = package / "components"

    parts = [
        f"/*! Prism {version} (https://prismjs.com), MIT License, see LICENSE next to this file. "
        "Bundled by rust/scripts/vendor-prism.py. */\n",
        (components / "prism-core.min.js").read_text(encoding="utf-8").strip(),
    ]
    for lang_id in sorted(languages):
        if lang_id == "meta":
            continue
        meta = languages[lang_id]
        require = as_list(meta.get("require"))
        after = [x for x in as_list(meta.get("optional")) + as_list(meta.get("modify")) if x not in require]
        code = (components / f"prism-{lang_id}.min.js").read_text(encoding="utf-8").strip()
        if "*/" in lang_id or "\n/*prism-component " in code:
            sys.exit(f"{lang_id}: cannot be bundled")
        parts.append(f"/*prism-component {lang_id} {','.join(require) or '-'} {','.join(after) or '-'}*/\n{code}")

    bundle = "\n".join(parts) + "\n"
    # The report inlines the bundle in a <script> element, which this would end.
    if "</script" in bundle.lower():
        sys.exit("the bundle contains </script")
    ASSETS.mkdir(parents=True, exist_ok=True)
    # UTF-8 and LF on any platform, as prism.rs reads it.
    (ASSETS / "prism.min.js").write_text(bundle, encoding="utf-8", newline="\n")
    shutil.copyfile(package / "LICENSE", ASSETS / "LICENSE")
    print(f"Prism {version}: {len(parts) - 2} languages, {len(bundle.encode())} bytes in {ASSETS / 'prism.min.js'}")


if __name__ == "__main__":
    main()
