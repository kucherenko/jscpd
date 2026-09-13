# xml-report demo

Shows that the `xml` reporter produces well-formed XML even when a clone
contains bytes XML cannot represent (issue #375). All commands run from the
repository root at default thresholds; `xmllint` ships with macOS and
libxml2 on Linux.

| Directory | Contents | Default scan |
|-----------|----------|--------------|
| `escapes/` | Two JavaScript files sharing a function whose body holds real ANSI escape bytes (`0x1B`), a form feed (`0x0C`) and a `"]]>"` literal | `Found 1 clones.` |

## `escapes/`: a clone the XML report has to carry

`banner.js` and `banner-copy.js` differ only in the function name, so the
clone spans the whole body, escapes included.

```bash
jscpd fixtures/xml-report-demo --reporters xml,console --output report
# Clone found (javascript)
#  - escapes/banner-copy.js [5:33 - 18:2] (14 lines, 115 tokens)
#    escapes/banner.js [5:28 - 18:2]
# Found 1 clones.
# XML report saved to report/jscpd-report.xml

xmllint --noout report/jscpd-report.xml
# (no output: the document is well-formed)
```

Inside the report the escape and form-feed bytes are replaced by U+FFFD, the
replacement character, because XML 1.0 has no way to encode them, not even in
CDATA. Every `]]>` is split across two CDATA sections, so a parser reads the
snippet text back unchanged:

```bash
grep -c '<!\[CDATA\[' report/jscpd-report.xml
# 6

python3 -c "import xml.dom.minidom as m; m.parse('report/jscpd-report.xml'); print('ok')"
# ok
```

Before the fix (jscpd 5.2.0 and earlier) the same command wrote the bytes
verbatim and `xmllint` reported `PCDATA invalid Char value 27` on the first
escape and refused the file.
