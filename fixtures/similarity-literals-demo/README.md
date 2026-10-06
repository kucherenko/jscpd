# Literals in similar functions

`--similarity` compares whole functions by the shape of their syntax trees, and `--similarity-literals` decides what the literals in them add to that shape. This demo shows its four modes on Python files, on Python code blocks in a Markdown file, and on JavaScript files. Run the commands from the repository root. They use the default thresholds.

| Directory | What the files hold | Default scan | `--similarity 0.85` | `values` | `generic` | `omit` |
|---|---|---|---|---|---|---|
| `values/` | Two functions whose literals have other values of the same kinds | 0 clones | 1 clone | 0 clones | 1 clone | 1 clone |
| `categories/` | Two typed functions whose literals are of other kinds | 0 clones | 0 clones | 0 clones | 1 clone | 1 clone |
| `markdown/` | Two `python` fences of one guide with other values of the same kinds | 0 clones | 1 clone | 0 clones | 1 clone | 1 clone |
| `javascript/` | Two policy builders whose literals are of other kinds | 0 clones | 0 clones | 0 clones | 1 clone | 1 clone |

`--similarity 0.85` runs the default mode, `categories`. The last three columns add `--similarity-literals` with that mode to `--similarity 0.85`.

## `values/`

`billing.py` charges an invoice and `payouts.py` sends a payout. They are one function with other names and other values: the currency, the method, the retry count, the status, the timeouts and the ledger entry. Every literal keeps its kind, so a string stays a string and a number a number. By default only the kind counts, and the pair scores 1.00. In the `values` mode the values count too, and the score falls to 0.42. The docstrings differ as well, but a docstring is documentation and counts in no mode.

```bash
jscpd fixtures/similarity-literals-demo/values --similarity 0.85
# Clone found (python, similar (ast) ~1.00)
#  - billing.py [1:1 - 8:19] (8 lines, 84 tokens)
#    payouts.py [1:1 - 8:20]
# Found 1 clones.
jscpd fixtures/similarity-literals-demo/values --similarity 0.85 --similarity-literals values
# Found 0 clones.
```

`generic` and `omit` keep the pair at 1.00, since its literals already match by kind.

## `categories/`

`orders.py` and `members.py` move a record to another state. The order's status is `Literal["paid", "refunded"]` and the function returns `Literal["ok", "skipped"]`. The member's tier is `Literal[1, 2]` and the function returns `Literal[True, False]`. The bodies follow suit, with strings in one where the other has booleans and `None`. The annotations keep their structure in every mode; only the literals inside them follow it.

The kinds count by default, and the pair stays under 0.85. `generic` puts one marker in place of every literal and `omit` leaves literals out, so both report the pair at 1.00:

```bash
jscpd fixtures/similarity-literals-demo/categories --similarity 0.85
# Found 0 clones.
jscpd fixtures/similarity-literals-demo/categories --similarity 0.85 --similarity-literals generic
# Clone found (python, similar (ast) ~1.00)
#  - members.py [1:1 - 8:16] (8 lines, 64 tokens)
#    orders.py [1:1 - 8:16]
# Found 1 clones.
jscpd fixtures/similarity-literals-demo/categories --similarity 0.85 --similarity-literals omit
# Found 1 clones.
```

## `markdown/`

`pricing.md` shows one discount rule twice, in two `python` fences, with other rates and labels. jscpd reads each fence as Python, applies the same mode, and reports the pair at the lines of the Markdown file:

```bash
jscpd fixtures/similarity-literals-demo/markdown --similarity 0.85
# Clone found (python, similar (ast) ~1.00)
#  - pricing.md:python [6:1 - 13:17] (8 lines, 74 tokens)
#    pricing.md:python [19:1 - 26:17]
# Found 1 clones.
jscpd fixtures/similarity-literals-demo/markdown --similarity 0.85 --similarity-literals values
# Found 0 clones.
```

## `javascript/`

`retry-policy.js` and `cache-policy.js` build a frozen policy object. Wherever one has a literal, the other has a literal of another kind: a string for a number, a string for a boolean, `null` for a number. By default the pair scores 0.53, and with `generic` it scores 1.00:

```bash
jscpd fixtures/similarity-literals-demo/javascript --similarity 0.5
# Clone found (javascript, similar (ast) ~0.53)
#  - cache-policy.js [1:8 - 9:2] (9 lines, 65 tokens)
#    retry-policy.js [1:8 - 9:2]
# Found 1 clones.
jscpd fixtures/similarity-literals-demo/javascript --similarity 0.85 --similarity-literals generic
# Clone found (javascript, similar (ast) ~1.00)
#  - cache-policy.js [1:8 - 9:2] (9 lines, 65 tokens)
#    retry-policy.js [1:8 - 9:2]
# Found 1 clones.
```

## Whole directory

```bash
jscpd fixtures/similarity-literals-demo --similarity 0.85
# Found 2 clones.
jscpd fixtures/similarity-literals-demo --similarity 0.85 --similarity-literals values
# Found 0 clones.
jscpd fixtures/similarity-literals-demo --similarity 0.85 --similarity-literals generic
# Found 4 clones.
jscpd fixtures/similarity-literals-demo --similarity 0.85 --similarity-literals omit
# Found 4 clones.
jscpd fixtures/similarity-literals-demo --similarity-literals generic
# Warning: --similarity-literals generic has no effect without --similarity
# Found 0 clones.
```
