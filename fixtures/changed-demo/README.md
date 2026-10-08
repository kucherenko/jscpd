# changed demo

`--changed` reports only the clones of the files `git status` lists as
changed, matched against every file of the scan, and marks the ones HEAD
didn't have as `[NEW]`. The baseline of HEAD is saved to
`.jscpd-baseline.json` at the repository root on the first run and built
again after a commit. `--changed-only` scans the changed files alone.
Changes need a repository, so this demo builds one in a temporary directory
instead of shipping files. Run it at default thresholds.

| Step | Change | `--changed` | `--changed-only` |
|------|--------|-------------|------------------|
| 1 | none: `orders.js` and `invoices.js` share `summarize()` in HEAD | 0 clones, baseline saved | |
| 2 | new `ring.js` copies the `Queue` class of `queue.js` | 1 clone, new | 0 clones |
| 3 | `invoices.js` gets one more line | 2 clones, 1 new | |
| 4 | commit | 0 clones, baseline rebuilt | |
| 5 | new `circle.js` and `deque.js` copy `Queue` too | 3 clones, new | 1 clone, new |

## Build the repository

```bash
demo=$(mktemp -d) && cd "$demo" && git init -q && mkdir src
git config user.email demo@example.com && git config user.name demo && git config commit.gpgsign false
cat > src/orders.js <<'EOF'
export function summarize(orders, minimum, title) {
  const large = orders.filter((order) => order.amount >= minimum);
  const total = large.reduce((sum, order) => sum + order.amount, 0);
  const average = large.length ? total / large.length : 0;
  const line = `${title}: ${large.length} orders, ${average.toFixed(2)} each`;
  console.log(line);
  return { large, total, average, line };
}
EOF
cp src/orders.js src/invoices.js
cat > src/queue.js <<'EOF'
export class Queue {
  constructor(capacity) {
    this.items = new Array(capacity);
    this.head = 0;
    this.tail = 0;
  }

  push(item) {
    if (this.tail - this.head === this.items.length) throw new Error("full");
    this.items[this.tail++ % this.items.length] = item;
  }

  shift() {
    if (this.tail === this.head) return undefined;
    return this.items[this.head++ % this.items.length];
  }
}
EOF
git add -A && git commit -q -m 'orders, invoices and a queue'
```

## 1. Nothing changed: the baseline is saved

```bash
jscpd --changed src --no-colors --no-tips
# Baseline .jscpd-baseline.json saved from HEAD <commit>: 1 fingerprints
# No duplicates found.
# ...
# Found 0 clones.
```

The tree is HEAD, so the run itself gives the baseline: one fingerprint, the
clone of `orders.js` and `invoices.js`. Neither file changed, so the clone is
not reported. `<commit>` is the short hash of HEAD, different on every
machine.

## 2. A new file copies an unchanged one

```bash
sed 's/Queue/Ring/' src/queue.js > src/ring.js

jscpd src --no-colors --no-tips
# Found 2 clones.

jscpd --changed src --no-colors --no-tips
# Clone found (javascript) [NEW]
#  - queue.js [1:20 - 17:2] (17 lines, 112 tokens)
#    ring.js [1:19 - 17:2]
# ...
# Found 1 clones (1 new).

jscpd --changed-only src --no-colors --no-tips
# No duplicates found.
# ...
# Found 0 clones.
```

`queue.js` didn't change, but `--changed` scans every file, so the copy in
the untracked `ring.js` is found. HEAD didn't have this clone, so it is new.
`--changed-only` scans `ring.js` alone and finds nothing to match it with.

## 3. A changed file with an old clone

```bash
printf '\nexport const currency = "EUR";\n' >> src/invoices.js

jscpd --changed src --no-colors --no-tips
# Clone found (javascript)
#  - invoices.js [1:1 - 8:2] (8 lines, 103 tokens)
#    orders.js [1:1 - 8:2]
# Clone found (javascript) [NEW]
#  - queue.js [1:20 - 17:2] (17 lines, 112 tokens)
#    ring.js [1:19 - 17:2]
# ...
# Found 2 clones (1 new).

jscpd --changed src --no-colors --no-tips --fail-on-new-clones 0
# ... the same two clones ...
# Found 2 clones (1 new).
# ERROR: jscpd found 1 new clones not in the baseline (allowed: 0)
# (exit code 1)
```

`invoices.js` changed now, so its clone is reported. The clone is in the
baseline, so it is not new and the gate fails only on the `Ring` copy.

## 4. A commit: the baseline is built again

```bash
git add src && git commit -q -m 'ring and currency'

jscpd --changed src --no-colors --no-tips
# Baseline .jscpd-baseline.json rebuilt for HEAD <commit> (was <commit>): 2 fingerprints
# No duplicates found.
# ...
# Found 0 clones.
```

The file names the commit it was built from, and HEAD has moved on. The new
HEAD has two clones, and nothing has changed since it.

## 5. Two new copies

```bash
sed 's/Queue/Circle/' src/queue.js > src/circle.js
sed 's/Queue/Deque/' src/queue.js > src/deque.js

jscpd --changed src --no-colors --no-tips
# Clone found (javascript) [NEW]
#  - circle.js [1:21 - 17:2] (17 lines, 112 tokens)
#    deque.js [1:20 - 17:2]
# Clone found (javascript) [NEW]
#  - circle.js [1:21 - 17:2] (17 lines, 112 tokens)
#    queue.js [1:20 - 17:2]
# Clone found (javascript) [NEW]
#  - circle.js [1:21 - 17:2] (17 lines, 112 tokens)
#    ring.js [1:19 - 17:2]
# ...
# Found 3 clones (3 new).

jscpd --changed-only src --no-colors --no-tips
# Clone found (javascript) [NEW]
#  - circle.js [1:21 - 17:2] (17 lines, 112 tokens)
#    deque.js [1:20 - 17:2]
# ...
# Found 1 clones (1 new).
```

The four copies of `Queue` make one group, reported as the pairs of
`circle.js`. `--changed-only` sees only the two new files, so it reports the
one clone between them.
