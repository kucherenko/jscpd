# Similar functions in Python and in Markdown code blocks

`--similarity` compares whole functions by the shape of their syntax trees. This demo shows it on Python files, on Python code blocks inside a Markdown file, and with `--similarity-identifiers role-aware`, which also compares the methods that calls invoke. Run the commands from the repository root. They use the default thresholds.

| Directory | What the files hold | Default scan | `--similarity 0.85` | Adding `--similarity-identifiers role-aware` |
|---|---|---|---|---|
| `python/` | A function and a copy with every name changed: variables, parameters, receivers, types | 0 clones | 1 clone | 1 clone |
| `methods/` | Two functions with one shape that call different methods | 0 clones | 1 clone | 0 clones |
| `markdown/` | A function and a renamed copy in two `python` fences of one guide | 0 clones | 1 clone | 1 clone |

## `python/`

`inventory.py` restocks an item and `library.py` returns a book. The second is the first with new names, so no run of 50 tokens repeats and the token scan finds nothing. The syntax trees are the same, and both modes report the pair.

```bash
jscpd fixtures/similarity-python-demo/python
# Found 0 clones.
jscpd fixtures/similarity-python-demo/python --similarity 0.85
# Clone found (python, similar (ast) ~1.00)
#  - inventory.py [1:1 - 9:16] (9 lines, 73 tokens)
#    library.py [1:1 - 9:16]
# Found 1 clones.
jscpd fixtures/similarity-python-demo/python --similarity 0.85 --similarity-identifiers role-aware
# Found 1 clones.
```

## `methods/`

`export.py` uploads a report and touches its record, and `cleanup.py` unlinks the file and drops the record. Both read the report the same way and write the same audit entry, so their trees have one shape. By default names do not count, and the pair scores 1.00. In role-aware mode the methods `upload` and `touch` differ from `unlink` and `drop`, and the score falls to 0.80.

```bash
jscpd fixtures/similarity-python-demo/methods --similarity 0.85
# Clone found (python, similar (ast) ~1.00)
#  - cleanup.py [1:1 - 9:18] (9 lines, 69 tokens)
#    export.py [1:1 - 9:18]
# Found 1 clones.
jscpd fixtures/similarity-python-demo/methods --similarity 0.85 --similarity-identifiers role-aware
# Found 0 clones.
jscpd fixtures/similarity-python-demo/methods --similarity 0.5 --similarity-identifiers role-aware
# Clone found (python, similar (ast) ~0.80)
# Found 1 clones.
```

## `markdown/`

`guide.md` explains a rate limit with one `python` fence for a token bucket and one for prepaid credit. jscpd parses each fence as Python and reports the pair at the lines of the Markdown file.

```bash
jscpd fixtures/similarity-python-demo/markdown --similarity 0.85
# Clone found (python, similar (ast) ~1.00)
#  - guide.md:python [6:1 - 14:16] (9 lines, 67 tokens)
#    guide.md:python [20:1 - 28:16]
# Found 1 clones.
```

## Whole directory

```bash
jscpd fixtures/similarity-python-demo --similarity 0.85
# Found 3 clones.
jscpd fixtures/similarity-python-demo --similarity 0.85 --similarity-identifiers role-aware
# Found 2 clones.
```
