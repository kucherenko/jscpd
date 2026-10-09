# Built-in file names and extensions

jscpd picks the format of a file from a conventional file name such as `Dockerfile` or `Makefile`, and from the extension when no such name applies. This folder holds three pairs of copied files that jscpd 5.4.1 and earlier skipped unless you mapped them with `--formats-names` or `--formats-exts`. Run the commands from the repository root; they use the default thresholds.

| Directory | Files | Matched by | Clones |
|-----------|-------|------------|--------|
| `services/api`, `services/worker` | `Dockerfile` | file name, format `docker` | 1 |
| `services/api`, `services/worker` | `Makefile` | file name, format `makefile` | 1 |
| `lib/shop` | `invoice_totals.ex`, `refund_totals.ex` | extension `.ex`, format `elixir` | 1 |

[FORMATS.md](../../FORMATS.md) lists every built-in extension and file name.

## The whole folder

```bash
jscpd fixtures/file-names-demo
# Found 3 clones.
```

The summary table gets a `docker`, a `makefile` and an `elixir` row. With jscpd 5.4.1 the same command prints `Found 0 clones.` and warns that it analyzed no files.

The two Dockerfiles share their build stages. The API image ends with a health check and a port, the worker image with a concurrency flag. The Makefiles differ only in the `SERVICE` name, and the Elixir modules only in their names and doc strings.

## One format

`--format` accepts the format names from the table:

```bash
jscpd fixtures/file-names-demo --format docker
# Found 1 clones.
```

## A mapping of your own wins

Mappings you pass on the command line or in `.jscpd.json` take priority over the built-in ones. This command reads both Dockerfiles as plain text, so jscpd reports the Docker clone under `txt`:

```bash
jscpd fixtures/file-names-demo --formats-names "txt:Dockerfile"
# Found 3 clones.
```
