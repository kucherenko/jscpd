# Type-2 clones

Type-2 clones are code blocks that are identical once identifiers, literal
values or annotations are ignored. jscpd finds them with three opt-in flags
(config keys `ignoreIdentifiers`, `ignoreLiterals`, `ignoreAnnotations`).
Each subdirectory holds one pair that is invisible to a default scan and
becomes a clone with one flag. Commands run from the repository root at
default thresholds; the `Found N clones.` line is the console reporter's.

| Directory | Flag | Default scan | With the flag |
|-----------|------|--------------|---------------|
| `identifiers/` | `--ignore-identifiers` | 0 clones | 1 clone, `renamed` |
| `literals/` | `--ignore-literals` | 0 clones | 1 clone, `renamed` |
| `annotations/` | `--ignore-annotations` | 0 clones | 1 clone, `renamed` |

Clones found this way are reported with `kind: renamed` (`kind: exact` for
token-identical pairs), so exact and similar code stay distinguishable in
every reporter: the console prints `Clone found (javascript, renamed)`, the
JSON report carries `"kind"`, SARIF uses the rule `jscpd/similar-code`.

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

## All three together

```bash
jscpd fixtures/type2-demo
# Found 0 clones.

jscpd fixtures/type2-demo --ignore-identifiers --ignore-literals --ignore-annotations
# Found 3 clones.

jscpd fixtures/type2-demo --ignore-identifiers --ignore-literals --ignore-annotations --reporters json,silent --output report
# report/jscpd-report.json: every entry in "duplicates" has "kind": "renamed"
```

Normalized runs report more, and longer, clones than exact runs, so their
snippet fingerprints differ: keep a separate `--baseline` file for a
normalized configuration.
