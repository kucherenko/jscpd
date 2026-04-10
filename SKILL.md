---
name: jscpd
description: Detect and eliminate copy-paste duplication in source code using jscpd with the AI reporter.
---

# jscpd

Copy-paste detector for programming source code, supports 150+ languages. Use this skill to find duplicated code and refactor it away.

## Quick Start

```bash
# Run with ai reporter (compact output optimized for agents)
npx jscpd --reporters ai <path>

# With ignore patterns
npx jscpd --reporters ai --ignore "**/node_modules/**,**/dist/**" <path>

# Scope to specific formats
npx jscpd --reporters ai --format "javascript,typescript" <path>
```

## AI Reporter Output Format

The `ai` reporter produces compact, token-efficient output designed for agent consumption:

```
Clones:
src/ foo.ts:10-25 ~ bar.ts:42-57
src/utils/helpers.ts:100-120 ~ src/utils/other.ts:5-25
---
3 clones · 4.2% duplication
```

Each line represents one clone pair:
- **Same file**: `path/file.ts 10-25 ~ 45-60` (shared path shown once)
- **Same directory**: `shared/prefix/ file-a.ts:10-25 ~ file-b.ts:42-57` (common prefix factored out)
- **Different paths**: `path/a.ts:10-25 ~ path/b.ts:42-57`

## Common Options

| Option | Description |
|--------|-------------|
| `--reporters ai` | Use the AI-optimized reporter (required for this skill) |
| `--min-tokens N` | Minimum tokens to consider a duplication (default: 50) |
| `--min-lines N` | Minimum lines to consider a duplication (default: 5) |
| `--threshold N` | Exit with error if duplication % exceeds N |
| `--ignore "glob"` | Ignore patterns (comma-separated) |
| `--format "list"` | Limit to specific languages (e.g. `typescript,javascript`) |
| `--pattern "glob"` | Glob pattern to select files |
| `--gitignore` | Respect .gitignore |
| `--silent` | Suppress output (useful with `--output` only) |

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
- Use `--min-lines 10` to filter noise and focus on meaningful duplications
