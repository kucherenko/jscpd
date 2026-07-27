---
name: dry-refactoring
description: Guided workflow to eliminate copy-paste duplication detected by jscpd. Refactor clones using extract function, module, constant, or base class strategies.
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

See the **[jscpd](../jscpd/SKILL.md)** skill for full option reference, including cross-format group syntax.

## Workflow

1. Run jscpd with `--reporters ai` on the target path
2. Parse each clone line to identify the two duplicated locations (file + line range)
3. Read both code fragments from the source files
4. Understand what the duplicated code does
5. Design a refactoring: extract a shared function, class, module, or constant
6. Apply the refactoring — update both locations and all other usages
7. Re-run jscpd to confirm the clone is eliminated
8. Repeat for remaining clones, highest-impact first

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

Always ensure:
- All call sites are updated, not just the two reported by jscpd
- Tests still pass after refactoring
- The extracted abstraction has a clear, descriptive name

## Tips

- Start with clones that have the highest line count — they have the most impact
- A clone between test files may indicate a missing test helper
- Clones across unrelated modules may signal a missing shared utility
- A cross-format clone (same logic in a `.js` and a `.ts` file, found with `--cross-formats`) often means code was ported without deleting the original — consolidate into one implementation (usually the TypeScript one) and update imports, rather than extracting a third shared copy
- Use `--min-lines 10` to filter noise and focus on meaningful duplications