# Type-2 clones

Type-2 clones are code blocks that are identical once identifiers, literal
values or annotations are ignored. jscpd finds them with three opt-in flags
(config keys `ignoreIdentifiers`, `ignoreLiterals`, `ignoreAnnotations`).
Each subdirectory holds one pair that is invisible to a default scan and
becomes a clone with one flag; `consistent/` holds three versions of one
function per language, for the two ways `--ignore-identifiers` matches names. Commands run from the repository root at
default thresholds; the `Found N clones.` line is the console reporter's.

| Directory | Flag | Default scan | With the flag |
|-----------|------|--------------|---------------|
| `identifiers/` | `--ignore-identifiers` | 0 clones | 1 clone, `renamed` |
| `literals/` | `--ignore-literals` | 0 clones | 1 clone, `renamed` |
| `annotations/` | `--ignore-annotations` | 0 clones | 1 clone, `renamed` |
| `annotation-types/` | `--ignore-annotations` keeps `@interface` | 0 clones | see below |
| `consistent/<language>/` | `--ignore-identifiers` | 0 clones | 1 clone, `renamed`; 2 with `--ignore-identifiers=blind` |

Clones found this way are reported with `kind: renamed` (`kind: exact` for
token-identical pairs), so exact and similar code stay distinguishable in
every reporter: the console prints `Clone found (javascript, renamed)`, the
JSON report carries `"kind"`, SARIF uses the rule `jscpd/renamed-code`.

## `identifiers/` — same logic, different names

`cart.js` and `basket.js` compute the same totals with every variable,
parameter and function renamed. Keywords (`for`, `const`, `return`) still have
to match, so only the structure is compared.

```bash
jscpd fixtures/type2-demo/identifiers
# Found 0 clones.

jscpd fixtures/type2-demo/identifiers --ignore-identifiers
# Clone found (javascript, renamed)
#  - basket.js [1:1 - 9:2] (9 lines, 57 tokens)
#    cart.js [1:1 - 9:2]
# Found 1 clones.
```

## `literals/` — same shape, different values

`limits-dev.js` and `limits-prod.js` are the same configuration object with
different numbers and strings. Numbers normalize to `$num` and strings to
`$str`, so a string never matches a number.

```bash
jscpd fixtures/type2-demo/literals
# Found 0 clones.

jscpd fixtures/type2-demo/literals --ignore-literals
# Clone found (javascript, renamed)
#  - limits-dev.js [1:1 - 13:3] (13 lines, 51 tokens)
#    limits-prod.js [1:1 - 13:3]
# Found 1 clones.
```

## `annotations/` — same methods, different annotations

`AlphaService.java` and `BetaService.java` have identical method bodies; the
annotations between them differ, which splits the default scan into two
runs that are each too short to report. `--ignore-annotations` drops
`@Name`, `@a.b.Name` and `@Name(...)` in Java, Kotlin, Scala, Groovy,
Python, Dart, Swift, JavaScript and TypeScript. Languages where `@` prefixes
a variable (Ruby, Perl, Razor, T-SQL) are untouched.

```bash
jscpd fixtures/type2-demo/annotations
# Found 0 clones.

jscpd fixtures/type2-demo/annotations --ignore-annotations
# Clone found (java, renamed)
#  - AlphaService.java [1:27 - 15:2] (15 lines, 59 tokens)
#    BetaService.java [1:26 - 15:2]
# Found 1 clones.
```

## `annotation-types/` — `@interface` is a declaration, not an annotation

`Marker.java` and `Tag.java` declare the same annotation type under two
names, each preceded by a real annotation use (`@Retention(...)`).
`--ignore-annotations` drops the *use* and keeps the `@interface` keyword:
`@` followed by a keyword never starts an annotation run. The token counts
pin that down: the `@Retention(RetentionPolicy.RUNTIME)` use is 7 tokens,
`@interface` is 2, and only the first 7 disappear.

```bash
jscpd fixtures/type2-demo/annotation-types
# Found 0 clones.

jscpd fixtures/type2-demo/annotation-types --ignore-identifiers
# Clone found (java, renamed)
#  - Marker.java [1:1 - 10:2] (10 lines, 62 tokens)
#    Tag.java [1:1 - 10:2]
# Found 1 clones.

jscpd fixtures/type2-demo/annotation-types --ignore-identifiers --ignore-annotations
# Clone found (java, renamed)
#  - Marker.java [1:1 - 10:2] (10 lines, 55 tokens)
#    Tag.java [1:1 - 10:2]
# Found 1 clones.
```

If `@interface` were stripped as well, the second clone would be 53 tokens.

## `consistent/` — one name for one name

Each language folder holds three versions of one function: `python`,
`javascript`, `typescript`, `java`, `csharp`, `go`, `rust`, and `markdown`,
where the code is Ruby in a fence. `two` renames every name of `one`, one
for one. `three` keeps the statements of `one` but has `x` wherever `one` has
`a`, `b` or `c`, so its data flows another way: `d = x + x * x` where `one`
has `d = a + b * c`.

`--ignore-identifiers` matches two fragments only when every name stands for
one name on the other side, and pairs `one` with `two` alone.
`--ignore-identifiers=blind` lets any name match any other, as jscpd 5.4 did,
and pairs `one` with both.

```bash
jscpd fixtures/type2-demo/consistent/python --ignore-identifiers=blind
# Clone found (python, renamed)
#  - one.py [1:1 - 8:13] (8 lines, 54 tokens)
#    three.py [1:1 - 8:13]
# Clone found (python, renamed)
#  - one.py [1:1 - 8:13] (8 lines, 54 tokens)
#    two.py [1:1 - 8:13]
# Found 2 clones.

jscpd fixtures/type2-demo/consistent/python --ignore-identifiers
# Clone found (python, renamed)
#  - one.py [1:1 - 8:13] (8 lines, 54 tokens)
#    two.py [1:1 - 8:13]
# Found 1 clones.
```

Every folder gives the same pairs. The clones are 53 tokens long in
`markdown/`, 54 in `python/`, 65 in `go/`, 69 in `javascript/`, 76 in `java/`
and `rust/`, 77 in `typescript/` and 79 in `csharp/`.

## All three together

```bash
jscpd fixtures/type2-demo
# Found 0 clones.

jscpd fixtures/type2-demo --ignore-identifiers --ignore-literals --ignore-annotations
# Found 12 clones.

jscpd fixtures/type2-demo --ignore-identifiers=blind --ignore-literals --ignore-annotations
# Found 20 clones.

jscpd fixtures/type2-demo --ignore-identifiers --ignore-literals --ignore-annotations --reporters json,silent --output report
# report/jscpd-report.json: every entry in "duplicates" has "kind": "renamed"
```

Normalized runs report more, and longer, clones than exact runs, so their
snippet fingerprints differ: keep a separate `--baseline` file for a
normalized configuration.
