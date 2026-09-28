# compare-demo

A billing module halfway through a port from Python to TypeScript. `python/` is the source of the port and `typescript/` is its target. `jscpd --compare` pairs each Python function with its TypeScript version and lists what each side has that the other lacks.

`--compare` pairs functions with the `--semantic` model, so download it once before running the commands below (CodeRankEmbed, 548 MB):

```bash
jscpd --semantic-download
```

Run the commands from this directory. From the repository root, jscpd would also load the repository's `.jscpd.json`.

| File | Functions | What happened in the port |
|---|---|---|
| `python/billing.py` | `line_total`, `apply_discount` | ported under the same names, in camelCase |
| | `tax_for_region` | ported as `salesTax`, with a different name |
| | `format_invoice_number` | ported as a one-line arrow function, too short to count on its own |
| | `due_date` | not ported yet |
| `python/shipping.py` | `shipping_cost` | ported |
| | `estimate_delivery_days` | not ported yet |
| `typescript/billing.ts` | `toCurrency` | new in TypeScript, no Python original |

## The port's progress

```bash
cd fixtures/compare-demo
jscpd --compare python typescript
```

```text
 71% 5 of 7 functions in python have a counterpart in typescript
 80% 4 of 5 functions in typescript have a counterpart in python

python
  billing.py   4 / 5  → billing.ts
  shipping.py  1 / 2  → shipping.ts

typescript
  billing.ts   3 / 4  → billing.py
  shipping.ts  1 / 1  → shipping.py

Only in python (2):
  billing.py:46   due_date                6 lines
  shipping.py:18  estimate_delivery_days  8 lines

Only in typescript (1):
  billing.ts:39  toCurrency  8 lines
```

The first line is the port's progress: five of the seven Python functions have a TypeScript version. "Only in python" is what is left to port. "Only in typescript" is code that exists only in TypeScript. The arrow after each file names the file on the other side that holds most of its counterparts.

`typescript` has 5 functions and not 6 because `formatInvoiceNumber` is shorter than the counting bar (`--min-tokens`, 30 with `--compare`, and `--min-lines`, 5). It is still found as the partner of `format_invoice_number`.

## Every pair

```bash
jscpd --compare python typescript -r console-full
```

The console report above, followed by:

```text
Pairs (5):
  billing.py:8 line_total              billing.ts:10 lineTotal            0.89
  billing.py:18 apply_discount         billing.ts:19 applyDiscount        0.93
  billing.py:28 tax_for_region         billing.ts:27 salesTax             0.87
  billing.py:38 format_invoice_number  billing.ts:36 formatInvoiceNumber  0.89 by name
  shipping.py:6 shipping_cost          shipping.ts:5 shippingCost         0.91
```

The number is the cosine similarity of the two functions. `tax_for_region` and `salesTax` pair on their code alone, since their names differ. `formatInvoiceNumber` is too short for the code match, so it pairs by name: the names are the same once case and underscores are ignored, and the code is similar enough.

## JSON and Markdown

```bash
jscpd --compare python typescript -r json,markdown -o report
```

```text
JSON report saved to report/jscpd-compare.json
Markdown report saved to report/jscpd-compare.md
```

The JSON report has one entry per side (`path`, `functions`, `matched`, `percentage`, `files` and `unmatched`) and the list of `pairs`, each with its two functions, its `similarity` and `matchedBy` (`code` or `name`).

## Without --compare

```bash
jscpd .
# Found 0 clones.
```

The two sides are in different languages, so token matching finds nothing to report.
