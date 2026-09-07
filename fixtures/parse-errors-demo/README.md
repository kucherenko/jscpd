# JavaScript files with parse errors

`valid.js` and `redeclared.js` share the same function. `redeclared.js` also
declares `saveProfile` a second time, which the oxc parser reports as a
redeclaration error. Before the fix for #1023 a JavaScript or TypeScript file
with any parse diagnostic was handed to the word-split fallback tokenizer,
so its tokens never matched an oxc-tokenized file: only the self-clone inside
`redeclared.js` was reported and both cross-file clones vanished.

Tokens come from the lexer, not the parser, so a recoverable diagnostic now
keeps the oxc token stream. Only a parser that gives up entirely (or yields
no tokens) still falls back. Commands run from the repository root at
default thresholds.

```bash
jscpd fixtures/parse-errors-demo
# Clone found (javascript)
#  - redeclared.js [1:1 - 6:88] (6 lines, 84 tokens)
#    redeclared.js [16:1 - 21:88]
# Clone found (javascript)
#  - redeclared.js [1:1 - 6:88] (6 lines, 84 tokens)
#    valid.js [1:1 - 6:88]
# Clone found (javascript)
#  - redeclared.js [7:72 - 12:2] (6 lines, 76 tokens)
#    valid.js [6:86 - 11:2]
# Found 3 clones.

jscpd fixtures/parse-errors-demo --max-gap-lines 1
# Clone found (javascript)
#  - redeclared.js [1:1 - 6:88] (6 lines, 84 tokens)
#    redeclared.js [16:1 - 21:88]
# Clone found (javascript, similar (gap) ~0.92)
#  - redeclared.js [1:1 - 12:2] (12 lines, 158 tokens)
#    valid.js [1:1 - 11:2]
# Found 2 clones.
```

Before the fix the first command printed only the self-clone (`Found 1
clones.`, with 86 tokens from the fallback tokenizer instead of 84).
