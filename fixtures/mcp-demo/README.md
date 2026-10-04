# mcp-demo

`jscpd --mcp` runs jscpd as an MCP server: an AI assistant starts it for a project and calls its tools to find duplicated code before it writes more. This demo is a small shop module with one pair of files for each kind of clone the token passes find. The commands below talk to the server the way an assistant does, one JSON-RPC request per line on stdin. Run them from the repository root. They use the default thresholds (`--min-tokens 50`, `--min-lines 5`) and print results with [jq](https://jqlang.org).

| Files | What changed in the copy | Kind |
|---|---|---|
| `src/orders.js`, `src/reports/orders.js` | nothing | `exact` (Type-1) |
| `src/shipping.js`, `src/returns.js` | every name and two strings | `renamed` (Type-2) |
| `src/invoice.js`, `src/print/invoice.js` | one line added in the middle | `similar` (Type-3), merged across the gap |

A plain scan finds three exact clones: the orders copy, and the invoice copy as two halves on either side of the added line. It does not see the renamed pair:

```bash
jscpd fixtures/mcp-demo
# Found 3 clones.
```

## One tool call from the shell

This function sends one `tools/call` request and prints the JSON the tool answers with:

```bash
mcp() {
  printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$2" "$3" |
    jscpd --mcp "$1" 2>/dev/null | jq -c '.result.structuredContent'
}
```

## What a plain run finds, and more on request

Without `kinds`, the tools report what `jscpd` reports with the same options, here the three exact clones:

```bash
mcp fixtures/mcp-demo get_statistics '{}' | jq -c '{kinds, byKind}'
# {"kinds":["exact"],"byKind":{"exact":3}}
```

`kinds` asks for more. With renamed and similar clones, the renamed pair shows up, and the two halves of the invoice become one `similar` clone, merged across the added line:

```bash
mcp fixtures/mcp-demo get_statistics '{"kinds":["exact","renamed","similar"]}' | jq -c .byKind
# {"exact":1,"gap":1,"renamed":1}
```

## A snippet before it is written

`check_duplication` compares code with the project. Here the code is `returns.js` itself, so it finds the file and its renamed twin:

```bash
mcp fixtures/mcp-demo check_duplication "$(jq -nc --rawfile code fixtures/mcp-demo/src/returns.js '{code: $code, format: "js", kinds: ["exact", "renamed"]}')" |
  jq -c '.duplications[] | {kind, file, fileStartLine, fileEndLine}'
# {"kind":"exact","file":"src/returns.js","fileStartLine":1,"fileEndLine":10}
# {"kind":"renamed","file":"src/shipping.js","fileStartLine":1,"fileEndLine":10}
```

## Semantic clones and two folders compared

Semantic clones (Type-4) and `compare_folders` need the embedding model, which jscpd downloads once (548 MB):

```bash
jscpd --semantic-download
```

The [semantic demo](../semantic-demo/README.md) holds eight rules written in both Rust and Svelte:

```bash
mcp fixtures/semantic-demo get_statistics '{"kinds":["semantic"]}' | jq -c .byKind
# {"semantic":8}
```

A snippet in another language finds the functions that do its job. `snippets/cart_totals.py` is a Python version of the shop's cart totals, and it matches the Rust original and the Svelte copy:

```bash
mcp fixtures/semantic-demo check_duplication "$(jq -nc --rawfile code fixtures/mcp-demo/snippets/cart_totals.py '{code: $code, format: "py", kinds: ["semantic"]}')" |
  jq -c '.duplications[] | {file, name, similarity}'
# {"file":"backend/src/pricing.rs","name":"cart_totals","similarity":0.889}
# {"file":"frontend/src/lib/components/CartSummary.svelte","name":"computeTotals","similarity":0.75}
```

`compare_folders` pairs the functions of two folders as [`--compare`](../compare-demo/README.md) does. It resolves a relative path against the folder the server scans:

```bash
mcp fixtures/compare-demo compare_folders '{"left":"python","right":"typescript"}' |
  jq -c '.code.sides[] | {path, matched, functions}'
# {"path":"fixtures/compare-demo/python","matched":5,"functions":7}
# {"path":"fixtures/compare-demo/typescript","matched":4,"functions":5}
```
