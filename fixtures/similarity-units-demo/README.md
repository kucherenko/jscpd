# Classes and module constants in `--similarity`

In Python, `--similarity` compares more units than functions. A class with code is a unit with its fields and methods, and so is an assignment at module level or in a class body, and a type alias. Each unit is compared only with units of its kind. When two classes pair, their methods are part of the pair and are not reported on their own. Run the commands from the repository root. They use the default thresholds.

| Directory | What the files hold | Default scan | `--similarity 0.85` |
|---|---|---|---|
| `classes/` | A class, a renamed copy of it, and a class of another shape with a copy of one method | 0 clones | 3 clones |
| `config/` | A router built by calls at module level, and a renamed copy in another module | 0 clones | 1 clone |
| `markdown/` | A class and a renamed copy in two `python` fences of one guide | 0 clones | 1 clone |
| `declarations/` | Two unrelated enums and two unrelated route tables of one length | 0 clones | 0 clones |

## `classes/`

`bookings.py` keeps hotel bookings behind a cache, and `tickets.py` is the same class for a help desk with every name changed. No run of 50 tokens repeats, so the token scan finds nothing. With `--similarity` the two classes pair as one clone, and their `load` and `cancel` methods, which would pair on their own, are part of it. `guests.py` holds a class of another shape: it has a clock, a `check_in` method and no table. Its `load` is a copy of the one in the other two files, so it pairs with each of them as a function.

```bash
jscpd fixtures/similarity-units-demo/classes
# Found 0 clones.
jscpd fixtures/similarity-units-demo/classes --similarity 0.85
# Clone found (python, similar (ast) ~1.00)
#  - bookings.py [4:1 - 27:23] (24 lines, 144 tokens)
#    tickets.py [4:1 - 27:22]
# Clone found (python, similar (ast) ~1.00)
#  - bookings.py [13:5 - 19:23] (7 lines, 57 tokens)
#    guests.py [9:5 - 15:21]
# Clone found (python, similar (ast) ~1.00)
#  - guests.py [9:5 - 15:21] (7 lines, 57 tokens)
#    tickets.py [13:5 - 19:22]
# Found 3 clones.
```

The JSON report says what each pair is in `unit`:

```bash
jscpd fixtures/similarity-units-demo/classes --similarity 0.85 --reporters json --output /tmp/units-demo
jq -r '.duplicates[] | "\(.unit) \(.firstFile.start)-\(.firstFile.end)"' /tmp/units-demo/jscpd-report.json
# class 4-27
# function 13-19
# function 9-15
```

## `config/`

`orders.py` and `invoices.py` each declare `ROUTES`, a router built from calls: its prefix, middleware, handlers and timeouts. The two assignments have one shape, so they pair as variables. A list, tuple, set or dict is a table of data and no unit, whatever it holds, as `declarations/` shows.

```bash
jscpd fixtures/similarity-units-demo/config
# Found 0 clones.
jscpd fixtures/similarity-units-demo/config --similarity 0.85
# Clone found (python, similar (ast) ~1.00)
#  - invoices.py [4:1 - 13:2] (10 lines, 64 tokens)
#    orders.py [4:1 - 13:2]
# Found 1 clones.
```

## `markdown/`

`guide.md` sends notifications by email in one `python` fence and by text message in the next. jscpd parses each fence as Python, and the two outbox classes pair at the lines of the Markdown file.

```bash
jscpd fixtures/similarity-units-demo/markdown
# Found 0 clones.
jscpd fixtures/similarity-units-demo/markdown --similarity 0.85
# Clone found (python, similar (ast) ~1.00)
#  - guide.md:python [6:1 - 14:25] (9 lines, 74 tokens)
#    guide.md:python [20:1 - 28:28]
# Found 1 clones.
```

## `declarations/`

`colors.py` and `statuses.py` have nothing in common but their shapes: an enum of sixteen members each, and a list of five routes each. Names do not count, so as units they would match. An enum only declares its members and a list is a table of data, so neither is a unit, and `--similarity` leaves them alone.

```bash
jscpd fixtures/similarity-units-demo/declarations --similarity 0.85
# Found 0 clones.
```

## Whole directory

```bash
jscpd fixtures/similarity-units-demo
# Found 0 clones.
jscpd fixtures/similarity-units-demo --similarity 0.85
# Found 5 clones.
```
