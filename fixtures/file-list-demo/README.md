# Files named one by one

Pre-commit hooks, lint-staged and build systems such as Bazel pass jscpd a list of files instead of a folder. jscpd treats each file named on the command line as its own scan root. A file root gets the same rules as a file found in a folder: the format filters, `--pattern`, `--ignore` and `--max-size`. The ignore files (`.gitignore`, `.ignore`) don't apply to it: jscpd reads a file you name even when `.gitignore` lists it. A named symlink is skipped unless `--follow-symlinks` is set, as it is in a walk.

Up to jscpd 5.4.3 every path started its own pool of walker threads, so 3,000 files named one by one took about a hundred times longer than their folder (issue #1306). A file root now skips the walker.

Run the commands from the repository root. They use the default thresholds.

| Command | Files analyzed | Clones |
|---------|----------------|--------|
| the folder `src` | 3 | 1 |
| the three files of `src` | 3 | 1 |
| two files, one of them matched by `--ignore` | 1 | 0 |
| three files, `--pattern` matching one | 1 | 0 |

`src/shipping-quote.js` and `src/return-quote.js` price a parcel with the same code under two names. `src/parcel-id.js` holds unrelated code.

## The folder and its files

```bash
jscpd fixtures/file-list-demo/src
# Found 1 clones.

jscpd fixtures/file-list-demo/src/shipping-quote.js \
  fixtures/file-list-demo/src/return-quote.js \
  fixtures/file-list-demo/src/parcel-id.js
# Found 1 clones.
```

Both commands report one clone: lines 1 to 24 of `return-quote.js` and of `shipping-quote.js`.

## Filters on files named one by one

```bash
jscpd fixtures/file-list-demo/src/shipping-quote.js \
  fixtures/file-list-demo/src/return-quote.js --ignore '**/return-*.js'
# Found 0 clones.

jscpd fixtures/file-list-demo/src/shipping-quote.js \
  fixtures/file-list-demo/src/return-quote.js \
  fixtures/file-list-demo/src/parcel-id.js --pattern '**/shipping-*.js'
# Found 0 clones.
```

In a glob, `*` also matches `/`. So `**/s*.js` keeps `src/return-quote.js` too, through its folder name `src`.

## Timing a long file list

This loop writes 3,000 small files into a temporary folder and scans them both ways:

```bash
dir=$(mktemp -d) && mkdir -p "$dir/src"
for d in $(seq 1 30); do for f in $(seq 1 100); do
  printf 'export function f%d_%d(a, b) {\n  const x = a + %d;\n  return x * b - %d;\n}\n' \
    $d $f $f $d > "$dir/src/m${d}_$f.js"
done; done
time jscpd --silent "$dir/src"
time (find "$dir/src" -name '*.js' | xargs jscpd --silent)
```

The files are below the default thresholds, so both runs warn that they analyzed no files. The loop measures how long it takes to collect the files. On an 8-core machine the folder takes about 0.05 s. The file list took about 8 s with jscpd 5.4.3 and takes about 0.3 s now.
