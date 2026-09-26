# Windows line endings (CRLF)

`checkout_eu.py` and `checkout_us.py` are stored with Windows line endings,
`\r\n`, and the `.gitattributes` next to them keeps them that way on every
checkout. They are the same checkout module except for one line, the tax
rate. Python goes through the generic tokenizer. Up to jscpd 5.3.2, that
tokenizer moved one byte behind for every CRLF line, so everything that maps a
token back to the file was off by the number of lines above it: the positions
in the JSON report and the tokens an `--ignore-pattern` match removes. All
commands run from the repository root with default thresholds.

| Command | Result | Up to 5.3.2 |
|---------|--------|-------------|
| `jscpd fixtures/crlf-demo` | 2 clones, split at the tax rate | same |
| `--ignore-pattern 'tax_rate = [0-9.]+'` | 1 clone, lines 1-30, 165 tokens | 164 tokens |
| `--ignore-pattern discount` | first clone 79 tokens | 70 tokens: other tokens removed |
| JSON `endLoc` of the first clone | `position` 500 | 486, 14 bytes short at line 15 |

## The clone and the differing line

Line 15 sets `tax_rate = 0.20` in one file and `tax_rate = 0.07` in the other,
so a default scan finds the code above it and the code below it as two clones.

```bash
jscpd fixtures/crlf-demo
# Found 2 clones.

jscpd fixtures/crlf-demo --ignore-pattern 'tax_rate = [0-9.]+'
# Found 1 clones.  (checkout_eu.py [1:1 - 30:27], 30 lines, 165 tokens)
```

## Positions in the JSON report

`position` is a byte offset into the file. Line 15, column 14 of
`checkout_eu.py` is byte 500: fourteen CRLF lines of two line-ending bytes
each come before it.

```bash
jscpd fixtures/crlf-demo --reporters json --output /tmp/crlf-demo
jq -c '.duplicates[0].firstFile.endLoc' /tmp/crlf-demo/jscpd-report.json
# {"column":14,"line":15,"position":500}
```

## Removing a word everywhere

`discount` occurs on several lines of the first clone. Removing it takes the
same tokens out of both files, and the first clone keeps its other 79 tokens.
Up to 5.3.2 the removal landed a byte behind per line, and the clone counted
70.

```bash
jscpd fixtures/crlf-demo --ignore-pattern discount
# Found 2 clones.  (the first: 15 lines, 79 tokens)
```
