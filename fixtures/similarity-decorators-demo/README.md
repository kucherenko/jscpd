# Decorators in `--similarity`

`--similarity-decorators` decides how the decorators of a Python function or class count when `--similarity` compares it. `omit`, the default, leaves them out. `names` adds the name of each decorator, `get` for `@router.get(...)`, without its arguments. `full` adds the name and compares each decorator whole, its arguments too. In every mode the fragment starts at `def` and the size limits read the code without decorators. Run the commands from the repository root. They use the default thresholds.

| Directory | What the files hold | `omit` | `names` | `full` |
|---|---|---|---|---|
| `routes/` | A route handler and a renamed copy behind another route | ~1.00 | ~0.94 | ~0.83 |
| `tests/` | A test and a renamed copy, each with a `parametrize` table of its own length | ~1.00 | ~1.00 | ~0.95 |

Without `--similarity` neither directory has a clone: the copies have other names, so no run of 50 tokens repeats.

## `routes/`

`orders.py` reads an order at `@router.get("/orders/{order_id}")`, and `invoices.py` does the same for an invoice at `@router.delete("/invoices/{invoice_id}", status_code=204)`. The bodies differ in names only. Left out, the decorators change nothing, and the pair scores 1.00. By name, `get` is no `delete`, and the score falls to 0.94. Whole, the second decorator's extra argument counts too, and the score falls to 0.83, below `--similarity 0.85`.

```bash
jscpd fixtures/similarity-decorators-demo/routes --similarity 0.85
# Clone found (python, similar (ast) ~1.00)
#  - invoices.py [7:1 - 13:19] (7 lines, 64 tokens)
#    orders.py [7:1 - 13:17]
# Found 1 clones.
jscpd fixtures/similarity-decorators-demo/routes --similarity 0.85 --similarity-decorators names
# Clone found (python, similar (ast) ~0.94)
#  - invoices.py [7:1 - 13:19] (7 lines, 64 tokens)
#    orders.py [7:1 - 13:17]
# Found 1 clones.
jscpd fixtures/similarity-decorators-demo/routes --similarity 0.85 --similarity-decorators full
# Found 0 clones.
jscpd fixtures/similarity-decorators-demo/routes --similarity 0.8 --similarity-decorators full
# Clone found (python, similar (ast) ~0.83)
#  - invoices.py [7:1 - 13:19] (7 lines, 64 tokens)
#    orders.py [7:1 - 13:17]
# Found 1 clones.
```

## `tests/`

`test_prices.py` and `test_rates.py` run one test over a table of cases: three cases in the first, two in the second. Both decorators are `parametrize`, so the names mode sees no difference. The whole decorators differ by a row, and the score falls to 0.95. The size limits read the code of a test without its decorator, so a short test does not pass `--min-tokens` on the strength of its table, and the table lines do not count as duplicated.

```bash
jscpd fixtures/similarity-decorators-demo/tests --similarity 0.85 --similarity-decorators names
# Clone found (python, similar (ast) ~1.00)
#  - test_prices.py [14:1 - 20:55] (7 lines, 55 tokens)
#    test_rates.py [13:1 - 19:53]
# Found 1 clones.
jscpd fixtures/similarity-decorators-demo/tests --similarity 0.85 --similarity-decorators full
# Clone found (python, similar (ast) ~0.95)
#  - test_prices.py [14:1 - 20:55] (7 lines, 55 tokens)
#    test_rates.py [13:1 - 19:53]
# Found 1 clones.
```

## Whole directory

```bash
jscpd fixtures/similarity-decorators-demo
# Found 0 clones.
jscpd fixtures/similarity-decorators-demo --similarity 0.85
# Found 2 clones.
```
