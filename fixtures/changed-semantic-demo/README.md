# changed + semantic demo

`--changed --semantic` reports the semantic clones of the files `git status` lists as changed. The semantic pass compares their functions with every function of the scan, but not the unchanged functions with each other. The clones it reports are the ones a scan of every file reports for the changed files, with the same similarities and the same `[NEW]` marks. Up to jscpd 5.4.3 the pass compared every function with every other one and dropped the rest afterwards, so a check of a few changed files took as long as a full scan (issue #1303).

The baseline of HEAD has to hold every clone, the semantic ones too, so the run that builds it compares every pair. That happens once per commit: on a clean tree the run uses its own clones, and on a changed tree it scans a checkout of HEAD. Later runs on the same commit compare only the changed functions, and a clean tree with its baseline saved embeds nothing. `--update-baseline` writes every clone, so it compares every pair too.

The demo copies [`semantic-demo`](../semantic-demo/README.md) into a temporary repository. The model runs inside jscpd: download it once with `jscpd --semantic-download` (548 MB). Run the commands at default thresholds.

| Step | Change | `--changed --semantic` |
|------|--------|------------------------|
| 1 | none | 0 clones, every pair compared once: the baseline of HEAD is saved with the 8 semantic clones |
| 2 | a comment in `backend/src/pricing.rs` | 1 clone: the cart totals of `pricing.rs` and `CartSummary.svelte`, not new |
| 3 | new `frontend/src/lib/slug.ts` repeats the slug rule of `backend/src/text.rs` | 2 clones, 1 new |

## Build the repository

```bash
demo=$(mktemp -d) && cp -R fixtures/semantic-demo/. "$demo" && cd "$demo"
git init -q && git config user.email demo@example.com && git config user.name demo && git config commit.gpgsign false
git add -A && git commit -q -m 'the shop'
jscpd . --semantic --no-colors --no-tips
# Found 8 clones.
```

## 1. Nothing changed

```bash
jscpd . --changed --semantic --no-colors --no-tips
# Baseline .jscpd-baseline.json saved from HEAD <commit>: 8 fingerprints
# Found 0 clones.
```

The next run on the same commit reads the saved baseline. Nothing changed, so it compares no function and embeds nothing:

```bash
jscpd . --changed --semantic --no-colors --no-tips
# Found 0 clones.
```

## 2. A comment in one file

jscpd embeds a function's code without its comments, so the vectors come from the cache. Only the pairs of `pricing.rs` are looked for:

```bash
printf '\n// Totals are in cents.\n' >> backend/src/pricing.rs
jscpd . --changed --semantic --no-colors --no-tips
# Clone found (rust, semantic ~0.73)
#  - backend/src/pricing.rs [34:1 - 58:2] (25 lines, 174 tokens)
#    frontend/src/lib/components/CartSummary.svelte:typescript [19:3 - 31:4]
# Found 1 clones.
```

## 3. A new file repeats a rule

`makeSlug` builds a slug with a character loop, as `slugify` in `backend/src/text.rs` does:

```bash
cat > frontend/src/lib/slug.ts <<'EOF'
export function makeSlug(title: string): string {
  let out = '';
  let dash = false;
  for (const ch of title.toLowerCase()) {
    if ((ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9')) {
      out += ch;
      dash = false;
    } else if (!dash && out.length > 0) {
      out += '-';
      dash = true;
    }
  }
  return dash ? out.slice(0, -1) : out;
}
EOF
jscpd . --changed --semantic --no-colors --no-tips
# Clone found (rust, semantic ~0.73)
#  - backend/src/pricing.rs [34:1 - 58:2] (25 lines, 174 tokens)
#    frontend/src/lib/components/CartSummary.svelte:typescript [19:3 - 31:4]
# Clone found (rust, semantic ~0.74) [NEW]
#  - backend/src/text.rs [6:1 - 27:2] (22 lines, 103 tokens)
#    frontend/src/lib/slug.ts [1:8 - 14:2]
# Found 2 clones (1 new).
```

`jscpd . --semantic` reports the same two pairs with the same similarities, beside the clones of the files that did not change.

## Time

The demo is too small to show a difference. The run that builds the baseline takes as long as a full scan. Issue #1303 measured a TypeScript codebase of 78,000 files and 250,000 functions with the embeddings cached: a check of 8 changed files took 588 s with jscpd 5.4.3 and 14 s with a prototype of this change.
