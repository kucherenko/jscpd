# fail-on-empty demo

Shows the exit codes jscpd uses when a scan cannot mean what a green run
usually means: nothing was analyzed, a path is missing, a format name is
wrong, or a reporter could not write its file (issue #1047). All commands run
from the repository root at default thresholds.

| Directory | Contents | Default scan | With `--fail-on-empty` |
|-----------|----------|--------------|------------------------|
| `code/` | Two JavaScript files sharing a discount loop | `Found 1 clones.`, exit 0 | same, exit 0 |
| `nothing/` | One JavaScript file holding only comments | `Found 0 clones.` plus a warning, exit 0 | `ERROR`, exit 1 |

## `code/`: a normal scan

`pricing.js` and `invoice.js` both contain the same tier table and discount
loop, so the default scan reports one clone. `--fail-on-empty` changes nothing
here because files were analyzed.

```bash
jscpd fixtures/fail-on-empty-demo/code
# Clone found (javascript)
#  - invoice.js [4:53 - 18:62] (15 lines, 119 tokens)
#    pricing.js [4:56 - 18:62]
# Found 1 clones.

jscpd fixtures/fail-on-empty-demo/code --fail-on-empty
# Found 1 clones.
# (exit 0)
```

## `nothing/`: the path exists, nothing gets analyzed

`only-comments.js` contains comments only. JavaScript comments produce no
detection tokens, so the file is below `--min-tokens` and the scan analyzes
zero files. By default that is a warning and the run still exits 0, so an
intentionally empty tree does not break a pipeline. `--fail-on-empty` (config
key `failOnEmpty`) turns it into an error for CI jobs where an empty result
means a misconfigured scan. Reports are written before the check runs.

```bash
jscpd fixtures/fail-on-empty-demo/nothing
# Found 0 clones.
# Warning: jscpd analyzed no files: check the paths and the --format, --ignore and --pattern filters
# (exit 0)

jscpd fixtures/fail-on-empty-demo/nothing --fail-on-empty
# Found 0 clones.
# ERROR: jscpd analyzed no files (--fail-on-empty): check the paths and the --format, --ignore and --pattern filters
# (exit 1)
```

## Errors that exit 1 on their own

These do not need a flag. Each one used to write an empty report and exit 0.

```bash
# A format name that is not in `jscpd --list`
jscpd fixtures/fail-on-empty-demo/code --format nosuchlang
# Error: --format: 'nosuchlang' is not a supported format (run with --list to see supported formats)
# (exit 1)

# A scan path that does not exist
jscpd fixtures/fail-on-empty-demo/missing
# Error: path does not exist: fixtures/fail-on-empty-demo/missing
# (exit 1)

# A reporter that cannot write its output (here the output directory is a file)
jscpd fixtures/fail-on-empty-demo/code --reporters json --output fixtures/fail-on-empty-demo/code/pricing.js/report
# Reporter 'json' error: I/O error in reporter: Not a directory (os error 20)
# ERROR: a reporter failed to write its output (see the message above)
# (exit 1)
```

Formats introduced with `--formats-exts` or `--formats-names` are accepted by
`--format`; only names that exist nowhere are rejected.

## Whole directory

```bash
jscpd fixtures/fail-on-empty-demo
# Found 1 clones.
```
