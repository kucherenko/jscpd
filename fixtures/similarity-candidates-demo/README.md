# Candidates of `--similarity`

`--similarity-candidates` and `--similarity-skip-tests` decide which units `--similarity` compares. By default it compares every function, class, variable and type alias it finds. `definitions` keeps the units at the top of a module or in a class body: a function or class declared in a function, as a closure or a callback, is part of that function and is not compared on its own. A class starts a scope of its own, so the methods of a class declared in a function are still compared. `--similarity-skip-tests` leaves out test code: pytest and unittest tests, the functions passed to `it`, `test`, `describe` and the hooks of Jest, Vitest or Mocha, and everything in them. Run the commands from the repository root. They use the default thresholds.

| Directory | What the files hold | Default | With the option |
|---|---|---|---|
| `nested/` | Two exports with a nested row helper and a local batching class, renamed | 2 clones: the helpers and the classes | 1 clone with `definitions`: the methods of the classes |
| `tests/` | Two Python functions, their pytest tests and two Vitest tests, renamed | 3 clones | 1 clone with `--similarity-skip-tests`: the functions |

Without `--similarity` neither directory has a clone: the copies have other names, so no run of 50 tokens repeats.

## `nested/`

`billing.py` exports invoices to CSV and `shipping.py` publishes parcels to a queue. Each declares a helper that turns one record into a row, and a class that sends rows in batches. The outer functions do different work around them and do not pair. By default the helpers pair, and so do the classes, whose pair holds the pair of their methods. With `definitions` the helpers and the classes are part of the code of the outer functions, and the methods that send a batch pair on their own.

```bash
jscpd fixtures/similarity-candidates-demo/nested --similarity 0.85
# Clone found (python, similar (ast) ~1.00)
#  - billing.py [5:5 - 10:68] (6 lines, 76 tokens)
#    shipping.py [5:5 - 10:82]
# Clone found (python, similar (ast) ~1.00)
#  - billing.py [12:5 - 23:34] (12 lines, 81 tokens)
#    shipping.py [12:5 - 23:35]
# Found 2 clones.
jscpd fixtures/similarity-candidates-demo/nested --similarity 0.85 --similarity-candidates definitions
# Clone found (python, similar (ast) ~1.00)
#  - billing.py [18:9 - 23:34] (6 lines, 52 tokens)
#    shipping.py [18:9 - 23:35]
# Found 1 clones.
```

## `tests/`

`orders.py` and `invoices.py` add up an order and an invoice the same way. `test_orders.py` and `test_invoices.py` test them with the same steps, and `cart.spec.js` and `wishlist.spec.js` are one Vitest test written twice. `--similarity-skip-tests` leaves the tests out and keeps the pair of the functions. Token clones in tests are still reported, use `--ignore` to leave test files out of the scan.

```bash
jscpd fixtures/similarity-candidates-demo/tests --similarity 0.85
# Clone found (javascript, similar (ast) ~1.00)
#  - cart.spec.js [4:42 - 11:2] (8 lines, 94 tokens)
#    wishlist.spec.js [4:43 - 11:2]
# Clone found (python, similar (ast) ~1.00)
#  - invoices.py [1:1 - 7:37] (7 lines, 69 tokens)
#    orders.py [1:1 - 7:41]
# Clone found (python, similar (ast) ~1.00)
#  - test_invoices.py [4:1 - 10:31] (7 lines, 85 tokens)
#    test_orders.py [4:1 - 10:26]
# Found 3 clones.
jscpd fixtures/similarity-candidates-demo/tests --similarity 0.85 --similarity-skip-tests
# Clone found (python, similar (ast) ~1.00)
#  - invoices.py [1:1 - 7:37] (7 lines, 69 tokens)
#    orders.py [1:1 - 7:41]
# Found 1 clones.
```

## Whole directory

```bash
jscpd fixtures/similarity-candidates-demo
# Found 0 clones.
jscpd fixtures/similarity-candidates-demo --similarity 0.85
# Found 5 clones.
jscpd fixtures/similarity-candidates-demo --similarity 0.85 --similarity-candidates definitions --similarity-skip-tests
# Found 2 clones.
```
