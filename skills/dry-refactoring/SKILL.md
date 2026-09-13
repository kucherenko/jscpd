---
name: dry-refactoring
description: Guided workflow to eliminate copy-paste duplication detected by jscpd. Refactor exact, renamed and near-miss clones using extract function, parameterize, module, constant, or base class strategies, starting from the hotspots the summary ranks.
---

# dry-refactoring

Guided workflow to eliminate copy-paste duplication in source code. Use after running [jscpd](../jscpd/SKILL.md) to detect clones.

## Prerequisites

First, run jscpd to identify duplications:

```bash
npx jscpd --reporters ai <path>
```

In codebases that mix related formats (e.g. JavaScript and TypeScript), add `--cross-formats` so clones spanning both are detected too:

```bash
npx jscpd --reporters ai --cross-formats "js-ts" <path>
```

On larger codebases, add `--summary` to get a refactoring-hotspot overview alongside the clone list — top files and folders with a `dup%` column showing how much of each file is duplicated:

```bash
npx jscpd --reporters ai --summary <path>
```

The default scan reports only **exact** copies. Two more passes find the copies that were edited after pasting; run them once the exact clones are dealt with, because they report more and longer clones:

```bash
# Type-2: renamed copies (other variable names, other constants), reported as "(renamed)"
npx jscpd --reporters ai --ignore-identifiers --ignore-literals <path>

# Type-3: near-miss copies (a few inserted lines, or JS/TS functions with the same structure),
# reported as "[~0.91 gap]" and "[~0.75 ast]"
npx jscpd --reporters ai --max-gap-lines 2 --similarity 0.8 <path>
```

See the **[jscpd](../jscpd/SKILL.md)** skill for full option reference, including cross-format group syntax, the clone-kind suffixes and how to read the summary.

## Workflow

1. Run jscpd with `--reporters ai` on the target path (add `--summary` on larger codebases to pick a starting point: files with high `dup%` and high token counts pay off most)
2. Parse each clone line to identify the two duplicated locations (file + line range) and its kind: no suffix is an exact copy, `(renamed)` differs only in names or values, `[~N gap]` has a few edited lines in the middle, `[~N ast]` is a function pair with the same structure
3. Read both code fragments from the source files
4. Understand what the duplicated code does, and for renamed and similar clones list exactly what differs between the two sides
5. Design a refactoring: extract a shared function, class, module, or constant; the kind decides the strategy (below)
6. Apply the refactoring — update both locations and all other usages
7. Re-run jscpd **with the same flags** to confirm the clone is eliminated and the `dup%` of the touched files went down; a clone that was `(renamed)` will not show in a default run, so check with `--ignore-identifiers` again
8. Repeat for remaining clones, highest-impact first: exact clones, then renamed, then similar

## Refactoring Strategies

**Extract function** — when the duplicate is a block of logic:
```ts
// Before: same block in two places
// After: shared function called from both places
```

**Extract module/utility** — when the duplicate spans multiple files in different domains:
```ts
// Move shared logic to a shared utility file and import it
```

**Extract constant or config** — when the duplicate is repeated data or configuration.

**Template/base class** — when the duplicate is structural (e.g., repeated class shape).

**Parameterize** — for `(renamed)` clones. The two sides are the same algorithm over different names or values, so the things that differ become parameters:
```ts
// Before: computeCartTotal(items) and computeBasketTotal(entries), same body, other names;
//         limits-dev.js and limits-prod.js, same shape, other numbers
// After: one function whose parameters are the identifiers that differed,
//        or one function reading the values that differed from a config object
```
A renamed clone whose only difference is a literal is a missing constant or config entry, not a missing function.

**Unify near-miss copies** — for `[~N gap]` clones. Read the unmatched lines: the gap is the one place the copies diverged, typically a guard, a log call or an extra field. Extract the common body and pass the divergence in:
```ts
// Before: saveUser and saveAccount, identical except one inserted validation line
// After: one saveRecord(record, { validate }) with the inserted line behind the option,
//        or the inserted line moved to the caller before the shared call
```
If the gap changes the meaning rather than adding a step, keep two functions but extract the shared halves.

**Merge similar functions** — for `[~N ast]` clones. The structure matches but names, literals and some statements do not. Diff the two functions first; the `ast` score tells how much is shared (`0.9` is a copy with one edit, `0.75` a copy with a couple of added statements plus renames). Extract the shared skeleton and inject what differs, as arguments, a strategy object, or a callback:
```ts
// Before: buildInvoice(order, customer, taxRate) and buildCreditNote(refund, account, vatRate):
//         same loop, same rounding, one extra guard and one extra log call in the second
// After: buildDocument(source, party, rate, { filter, onBuilt }) used by both
```
Below about `0.7` the pair usually shares an idiom, not an implementation; leave those alone unless the summary shows the file is a hotspot anyway.

Always ensure:
- All call sites are updated, not just the two reported by jscpd
- Tests still pass after refactoring
- The extracted abstraction has a clear, descriptive name
- The re-run uses the same detection flags as the run that found the clone

## Tips

- Start with clones that have the highest line count — they have the most impact
- Use the summary's `dup%` column to order the work: a large file with a high share of duplicated lines pays back first
- A clone between test files may indicate a missing test helper
- Clones across unrelated modules may signal a missing shared utility
- A cross-format clone (same logic in a `.js` and a `.ts` file, found with `--cross-formats`) often means code was ported without deleting the original — consolidate into one implementation (usually the TypeScript one) and update imports, rather than extracting a third shared copy
- Many `(renamed)` clones in one file usually mean one abstraction is missing, not many: look for the shared shape before extracting pair by pair
- `--similarity` only covers JavaScript and TypeScript today; for other languages rely on the exact and `--max-gap-lines` passes
- Use `--min-lines 10` to filter noise and focus on meaningful duplications
- Keep a separate `--baseline` per set of detection flags when gating CI: renamed and similar runs fingerprint clones differently from exact runs
