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
| `python/test_billing.py` | three pytest tests | ported to `typescript/billing.test.ts` as `it('…', () => …)` cases |
| | `test_due_date_skips_the_weekend` | not ported yet, like `due_date` itself |

## The port's progress

```bash
cd fixtures/compare-demo
jscpd --compare python typescript
```

```text
Code
 71% 5 of 7 functions in python have a counterpart in typescript
 80% 4 of 5 functions in typescript have a counterpart in python

python
  file         paired  similarity  counterpart
  billing.py   4 / 5   0.89        billing.ts
  shipping.py  1 / 2   0.91        shipping.ts

typescript
  file         paired  similarity  counterpart
  billing.ts   3 / 4   0.90        billing.py
  shipping.ts  1 / 1   0.91        shipping.py

Paired under other names (1):
  python                        typescript              similarity
  billing.py:28 tax_for_region  billing.ts:27 salesTax  0.87 high

Only in python (2):
  billing.py (1)
    46  due_date                6 lines
  shipping.py (1)
    18  estimate_delivery_days  8 lines

Only in typescript (1):
  billing.ts (1)
    39  toCurrency  8 lines

Tests
 75% 3 of 4 tests in python have a counterpart in typescript
100% 3 of 3 tests in typescript have a counterpart in python

python
  file             paired  similarity  counterpart
  test_billing.py  3 / 4   0.87        billing.test.ts

typescript
  file             paired  similarity  counterpart
  billing.test.ts  3 / 3   0.87        test_billing.py

Only in python (1):
  test_billing.py (1)
    30  test_due_date_skips_the_weekend  7 lines
```

The report has two blocks, one for the code and one for the tests, and each measures its own progress. Five of the seven Python functions have a TypeScript version, and three of the four Python tests do. "Only in python" is what is left to port: `due_date`, `estimate_delivery_days`, and the test of `due_date`. "Only in typescript" is code that exists only in TypeScript.

jscpd tells a test by the conventions of its language, so it reads `test_billing.py` as a pytest file and `billing.test.ts` as a Vitest one. A test pairs only with a test, so the tests of `line_total` never stand in for `line_total` itself.

The file tables give each file's paired functions, the mean similarity of their pairs, and the file on the other side that holds most of the counterparts. "Paired under other names" lists the pairs whose names differ even once case, underscores, spaces and punctuation are ignored, the ones nobody would find by searching for a name: here `tax_for_region`, ported as `salesTax`. Each pair has its cosine similarity and a level, `high`, `medium` or `low`, on the scale of the model. `high` is almost always the same function. For `low`, read both, since related code pairs there too. A file with low pairs shows how many, as in `0.62, 1 low`. The "Only in" lists group the functions by file. In a terminal the report is in colour, and `--no-colors` prints it as shown here.

The code block of `typescript` has 4 functions and not 5 because `formatInvoiceNumber` is shorter than the counting bar (`--min-tokens`, 30 with `--compare`, and `--min-lines`, 5). It is still found as the partner of `format_invoice_number`.

## Every pair

```bash
jscpd --compare python typescript -r console-full
```

The console report above, with the pairs of each block at its end:

```text
Code
Pairs (5):
  python                               typescript                         similarity
  billing.py:8 line_total              billing.ts:10 lineTotal            0.89 high
  billing.py:18 apply_discount         billing.ts:19 applyDiscount        0.93 high
  billing.py:28 tax_for_region         billing.ts:27 salesTax             0.87 high
  billing.py:38 format_invoice_number  billing.ts:36 formatInvoiceNumber  0.89 high   by name
  shipping.py:6 shipping_cost          shipping.ts:5 shippingCost         0.91 high

Tests
Pairs (3):
  python                                                       typescript                                              similarity
  test_billing.py:8 test_line_total_sums_quantity_times_price  billing.test.ts:6 line total sums quantity times price  0.84 high
  test_billing.py:15 test_line_total_rejects_a_zero_quantity   billing.test.ts:13 line total rejects a zero quantity   0.91 high
  test_billing.py:23 test_discount_is_capped                   billing.test.ts:19 discount is capped                   0.84 high
```

`tax_for_region` and `salesTax` pair on their code alone, since their names differ. `formatInvoiceNumber` is too short for the code match, so it pairs by name, marked `by name`: the names are the same once case and underscores are ignored, and the code is similar enough.

The TypeScript tests are `it('line total sums quantity times price', () => …)` callbacks. jscpd names such a test case after its title, and it pairs with the pytest function `test_line_total_sums_quantity_times_price`. jscpd does not list the pair as renamed, because it compares test names without case, underscores, spaces, punctuation and the leading `test_`.

## Tests alone

A `--pattern` that picks the test files of both languages compares the tests alone:

```bash
jscpd --compare python typescript --pattern "**/{test_*.py,*.test.ts}"
```

```text
Tests
 75% 3 of 4 tests in python have a counterpart in typescript
100% 3 of 3 tests in typescript have a counterpart in python

python
  file             paired  similarity  counterpart
  test_billing.py  3 / 4   0.87        billing.test.ts

typescript
  file             paired  similarity  counterpart
  billing.test.ts  3 / 3   0.87        test_billing.py

Only in python (1):
  test_billing.py (1)
    30  test_due_date_skips_the_weekend  7 lines
```

## JSON and Markdown

```bash
jscpd --compare python typescript -r json,markdown -o report
```

```text
JSON report saved to report/jscpd-compare.json
Markdown report saved to report/jscpd-compare.md
```

The JSON report has one entry per side (`path`, `functions`, `matched`, `percentage`, `files`, `unmatched` and `readyToPort`) and the list of `pairs`, each with its two functions, its `similarity`, its `level`, `renamed` (whether the names differ) and `matchedBy` (`code` or `name`). Each file has its mean `similarity` and `lowPairs`.

## Migration map

```bash
jscpd --compare python typescript -r html -o report
```

```text
HTML report saved to report/jscpd-compare.html
```

Open `report/jscpd-compare.html` in a browser; it needs no network. The map opens with code and tests together, code as circles and tests as squares, Python in orange on the left and TypeScript in green on the right. The ported Python functions and tests line the channel, each facing its TypeScript counterpart across a dotted bridge, dark blue since every pair here is a close match. `due_date` and `estimate_delivery_days` keep to the far left as empty circles with nothing ported, and a dark ring marks both as ready to port, since neither calls a function that still lacks a counterpart. Their test, `test_due_date_skips_the_weekend`, waits beside them as an empty square. `toCurrency` sits at the far right, since it exists only in TypeScript. Hover a mark to light up its calls and its pair, and click it to list them. The Table tab lists the same as rows, in the order of the Python files: each function and test with its TypeScript counterpart and their similarity, `due_date` and `estimate_delivery_days` as ready to port, their test as not ported, and `toCurrency` last, only in the target. Under both views, the page lists the functions ready to port, charts how alike the pairs are, and shows each folder's progress.

## Without --compare

```bash
jscpd .
# Found 0 clones.
```

The two sides are in different languages, so token matching finds nothing to report.
