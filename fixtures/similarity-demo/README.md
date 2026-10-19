# Similar functions by structure

`--similarity` compares functions and methods by their syntax trees. Each tree is normalized first: the names of the functions and methods it calls stay, and so do its operators, while local names, field names and literals become markers. A name has to stand for one name on the other side, so a copy that swaps the roles of two names scores lower than a renamed one. Two functions then score the share of subtrees their trees have in common, from 0 to 1. A ratio after the flag sets the lowest score reported, 0.8 when it is left out.

Run the commands from the repository root. They use the default thresholds.

| Directory | What the files hold | Default scan | `--similarity` |
|---|---|---|---|
| `renamed/` | A Python function and a copy with other names and other literals | 0 clones | 1 clone, 1.00 |
| `calls/` | Two Python functions of one shape that call other methods | 0 clones | 0 clones (1 at 0.6) |
| `names/` | A Python function, a renamed copy and a mirrored one | 0 clones | 1 clone, 1.00 (2 at 0.7) |
| `edited/` | A TypeScript function and a renamed copy with one more statement | 0 clones | 1 clone, 0.85 |
| `markdown/` | A Python function and a renamed copy in two code blocks of a guide | 0 clones | 1 clone, 1.00 |
| `test-files/` | Two renamed copies in `src/` and a third in a test file | 0 clones | 1 clone |
| `languages/` | One function in C, C++, C#, Clojure, Go, Java, Kotlin, PHP, Ruby, Rust, Scala and Swift, with a renamed copy each | 0 clones | 12 clones, 1.00 |

## `renamed/`

`shipping.py` prices a parcel and `insurance.py` prices a policy. Every local name differs and so does every literal, so no run of tokens repeats. The calls (`max`, `lookup`, `round`, `min`) and the operators are the same, and the pair scores 1.00.

```bash
jscpd fixtures/similarity-demo/renamed
# Found 0 clones.
jscpd fixtures/similarity-demo/renamed --similarity
# Clone found (python, similar (ast) ~1.00)
#  - insurance.py [1:1 - 8:26] (8 lines, 62 tokens)
#    shipping.py [1:1 - 8:29]
# Found 1 clones.
```

## `calls/`

`accounts.py` closes an account and `devices.py` retires a device. The statements have one shape, but the first calls `freeze` and `archive` where the second calls `revoke` and `remove`. Called names count, so the score drops to 0.66.

```bash
jscpd fixtures/similarity-demo/calls --similarity
# Found 0 clones.
jscpd fixtures/similarity-demo/calls --similarity 0.6
# Clone found (python, similar (ast) ~0.66)
#  - accounts.py [1:1 - 8:19] (8 lines, 62 tokens)
#    devices.py [1:1 - 8:18]
# Found 1 clones.
```

## `names/`

`clamp_low.py` raises the values below a floor, `limit_low.py` is the same function with other names, and `clamp_high.py` lowers the values above a ceiling: `if value < floor` became `if ceiling < value`. All three are made of the same markers, but in `clamp_high.py` the name that stood for the floor is compared with the one that stood for the value, so it pairs one name with two. Only the renamed copy scores 1.00; the mirrored one scores 0.74. With `--ignore-identifiers=blind` any name matches any other, as in jscpd 5.4, and all three score 1.00.

```bash
jscpd fixtures/similarity-demo/names --similarity
# Clone found (python, similar (ast) ~1.00)
#  - clamp_low.py [1:1 - 7:18] (7 lines, 33 tokens)
#    limit_low.py [1:1 - 7:16]
# Found 1 clones.
jscpd fixtures/similarity-demo/names --similarity 0.7
# Clone found (python, similar (ast) ~0.74)
#  - clamp_high.py [1:1 - 7:18] (7 lines, 33 tokens)
#    clamp_low.py [1:1 - 7:18]
# Clone found (python, similar (ast) ~1.00)
#  - clamp_low.py [1:1 - 7:18] (7 lines, 33 tokens)
#    limit_low.py [1:1 - 7:16]
# Found 2 clones.
jscpd fixtures/similarity-demo/names --similarity --ignore-identifiers=blind
# Found 2 clones.   (both ~1.00, clamp_high.py with each of the others)
```

## `edited/`

`refunds.ts` is `orders.ts` with new names and a `trace(refund.id)` call added to the loop. The added call changes the loop and the function around it, and the pair scores 0.85.

```bash
jscpd fixtures/similarity-demo/edited --similarity
# Clone found (typescript, similar (ast) ~0.85)
#  - orders.ts [1:8 - 14:2] (14 lines, 121 tokens)
#    refunds.ts [1:8 - 15:2]
# Found 1 clones.
```

## `markdown/`

`guide.md` shows a retried request and a retried upload in two `python` code blocks. Each block is read as Python, and the pair is reported at the lines of the guide.

```bash
jscpd fixtures/similarity-demo/markdown --similarity
# Clone found (python, similar (ast) ~1.00)
#  - guide.md:python [7:1 - 14:28] (8 lines, 59 tokens)
#    guide.md:python [20:1 - 27:29]
# Found 1 clones.
```

## `test-files/`

`src/discount.py` and `src/coupon.py` are renamed copies, and so is `tests/test_discount.py`. Test files are left out of `--similarity`, so only the pair in `src/` is reported. Whether a file is a test is read from its path below the scanned folder.

```bash
jscpd fixtures/similarity-demo/test-files --similarity
# Clone found (python, similar (ast) ~1.00)
#  - src/coupon.py [1:1 - 7:40] (7 lines, 80 tokens)
#    src/discount.py [1:1 - 7:40]
# Found 1 clones.
```

## `languages/`

One restocking function per language, in `stock.*`, and a renamed copy that refills a shelf, in `shelf.*`. A function is compared only with functions of its own language, C with C++.

```bash
jscpd fixtures/similarity-demo/languages --similarity
# Clone found (csharp, similar (ast) ~1.00)
#  - Shelf.cs [3:5 - 14:6] (12 lines, 61 tokens)
#    Stock.cs [3:5 - 14:6]
# ...
# Found 12 clones.
```

The `edn` reporter writes every pair to `jscpd-report.edn`, the pairs a token clone already reports too, with the size of both normalized trees:

```bash
jscpd fixtures/similarity-demo/renamed --similarity -r edn -o report
# {:candidates [
#  {:score 1.0
#   :language "python"
#   :left {:file "insurance.py", :start-line 1, :end-line 8}
#   :right {:file "shipping.py", :start-line 1, :end-line 8}
#   :left-nodes 112
#   :right-nodes 112}
# ]
#  :clones []}
```

## Whole directory

```bash
jscpd fixtures/similarity-demo --similarity
# Found 17 clones.
```
