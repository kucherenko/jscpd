# Decorators in `--similarity`

`--similarity-decorators` decides how the decorators of a Python function or class count when `--similarity` compares it. `omit`, the default, leaves them out. `names` adds the name of each decorator, `get` for `@router.get(...)`, without its arguments. `full` compares each decorator whole, its arguments too. Run the commands from the repository root. They use the default thresholds.

| Directory | What the files hold | `omit` | `names` | `full` |
|---|---|---|---|---|
| `routes/` | A route handler and a renamed copy behind another route | ~1.00 | ~0.94 | ~0.88 |
| `tests/` | A test and a renamed copy, each with a `parametrize` table of its own length | ~1.00 | ~1.00 | ~0.95 |

Without `--similarity` neither directory has a clone: the copies have other names, so no run of 50 tokens repeats.

## `routes/`

`orders.py` reads an order at `@router.get("/orders/{order_id}")`, and `invoices.py` does the same for an invoice at `@router.delete("/invoices/{invoice_id}", status_code=204)`. The bodies differ in names only. Left out, the decorators change nothing, and the pair scores 1.00. By name, `get` is no `delete`, and the score falls to 0.94. Whole, the second decorator's extra argument counts as well. When the decorators count, the fragment starts at them.

```bash
jscpd fixtures/similarity-decorators-demo/routes --similarity 0.85
# Clone found (python, similar (ast) ~1.00)
#  - invoices.py [7:1 - 13:19] (7 lines, 64 tokens)
#    orders.py [7:1 - 13:17]
# Found 1 clones.
jscpd fixtures/similarity-decorators-demo/routes --similarity 0.85 --similarity-decorators names
# Clone found (python, similar (ast) ~0.94)
#  - invoices.py [6:1 - 13:19] (8 lines, 64 tokens)
#    orders.py [6:1 - 13:17]
# Found 1 clones.
jscpd fixtures/similarity-decorators-demo/routes --similarity 0.85 --similarity-decorators full
# Clone found (python, similar (ast) ~0.88)
#  - invoices.py [6:1 - 13:19] (8 lines, 64 tokens)
#    orders.py [6:1 - 13:17]
# Found 1 clones.
jscpd fixtures/similarity-decorators-demo/routes --similarity 0.9 --similarity-decorators full
# Found 0 clones.
```

## `tests/`

`test_prices.py` and `test_rates.py` run one test over a table of cases: three cases in the first, two in the second. Both decorators are `parametrize`, so the names mode sees no difference. The whole decorators differ by a row, and the score falls to 0.95. The size limits read the code of a test without its decorator in every mode, so a short test does not pass `--min-tokens` on the strength of its table.

```bash
jscpd fixtures/similarity-decorators-demo/tests --similarity 0.85 --similarity-decorators names
# Clone found (python, similar (ast) ~1.00)
#  - test_prices.py [6:1 - 20:55] (15 lines, 55 tokens)
#    test_rates.py [6:1 - 19:53]
# Found 1 clones.
jscpd fixtures/similarity-decorators-demo/tests --similarity 0.85 --similarity-decorators full
# Clone found (python, similar (ast) ~0.95)
#  - test_prices.py [6:1 - 20:55] (15 lines, 55 tokens)
#    test_rates.py [6:1 - 19:53]
# Found 1 clones.
```

## Whole directory

```bash
jscpd fixtures/similarity-decorators-demo
# Found 0 clones.
jscpd fixtures/similarity-decorators-demo --similarity 0.85
# Found 2 clones.
```
