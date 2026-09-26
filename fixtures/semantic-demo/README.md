# Semantic clones (Type-4, experimental)

A semantic clone is code that does the same job as other code without
looking like it: other names, another algorithm, another language. It shows
up when a project has two halves and a rule the backend enforces is written
again for the frontend, and when two people in one codebase each solve the
same problem their own way. Token matching sees neither. `--semantic`
(config key `semantic`) embeds every function with a code embedding model
and reports two functions as a clone of kind `semantic` when each is the
other's closest match and their cosine similarity reaches
`--semantic-threshold` (0.6 by default).

This demo is a small shop in two languages. `backend/` is a Rust API,
`frontend/` a SvelteKit app, and eight rules exist on both sides: e-mail and
password checks, cart totals, slugs, article previews, page links, relative
dates and a card checksum. Two features also exist twice within one
language: the RSS feed in `backend/src/feeds.rs` builds slugs its own way,
and the newsletter form in `frontend/src/lib/validators.ts` has its own
e-mail check. Each side also has code with no counterpart, which must stay
out of the report: routes, database access, configuration and error mapping
in Rust, and a fetch wrapper, a debounce helper, a theme switch and a dialog
focus trap in the frontend.

## Setup

The model runs inside jscpd. It is downloaded once into the user cache
directory (322 MB from huggingface.co, checked against its pinned SHA-256):

```bash
jscpd --semantic-download
```

Commands run from the repository root at default thresholds. The first scan
embeds 38 functions, which takes a few seconds on a laptop. Vectors are
cached, so later runs embed only what changed.

| Directory | What it holds | Default scan | `--semantic` |
|-----------|---------------|--------------|--------------|
| `backend/` + `frontend/` | 8 rules written in both Rust and Svelte, 2 features written twice in one language, code with no counterpart | 0 clones | 10 semantic clones |

## Across languages

| Rule | Rust, `backend/src/` | Svelte, `frontend/src/lib/components/` | Similarity |
|------|----------------------|----------------------------------------|------------|
| Page links with gaps | `pagination.rs` `page_links` | `Pager.svelte` `visiblePages` | 0.64 |
| Card number checksum | `payments.rs` `is_valid_card_number` | `PaymentForm.svelte` `cardLooksValid` | 0.79 |
| Cart totals | `pricing.rs` `cart_totals` | `CartSummary.svelte` `computeTotals` | 0.78 |
| Title to slug | `text.rs` `slugify` | `ArticleCard.svelte` `toSlug` | 0.69 |
| Article preview | `text.rs` `excerpt` | `ArticleCard.svelte` `preview` | 0.68 |
| "3 days ago" | `time.rs` `relative_time` | `RelativeTime.svelte` `ago` | 0.65 |
| E-mail check | `validation.rs` `validate_email` | `SignupForm.svelte` `checkEmail` | 0.72 |
| Password strength | `validation.rs` `password_strength` | `SignupForm.svelte` `rate` | 0.78 |

No pair shares a name, and several share no structure either. The Rust
checksum walks the digits with an iterator and doubles every other one; the
Svelte version runs a `while` loop over a lookup table. `relative_time`
chains `match` guards; `ago` loops over a table of units. `validate_sku` in
`validation.rs` is shaped like `validate_email` but checks something else,
and it pairs with nothing.

## Within one language

| Feature | First | Second | Similarity |
|---------|-------|--------|------------|
| Title to slug | `backend/src/text.rs` `slugify`: a character loop | `backend/src/feeds.rs` `feed_item_slug`: split, filter, join | 0.87 |
| E-mail check | `SignupForm.svelte` `checkEmail`: step by step | `frontend/src/lib/validators.ts` `isEmail`: one regex | 0.78 |

A feature written three times makes three pairs when the three are equally
close. `checkEmail` pairs with `validate_email` across languages and with
`isEmail` within one.

## Commands

```bash
jscpd fixtures/semantic-demo
# Found 0 clones.

jscpd fixtures/semantic-demo --semantic
# Semantic clones (experimental): embedding 38 functions with jinaai/jina-embeddings-v2-base-code on this machine
# Clone found (rust, semantic ~0.87)
#  - backend/src/feeds.rs [10:1 - 22:2] (13 lines, 107 tokens)
#    backend/src/text.rs [6:1 - 27:2]
# Clone found (rust, semantic ~0.64)
#  - backend/src/pagination.rs [14:1 - 36:2] (23 lines, 172 tokens)
#    frontend/src/lib/components/Pager.svelte:typescript [12:3 - 30:4]
# ... eight more pairs ...
# Found 10 clones.
```

`--semantic-scope` keeps one kind of pair: `same` for the implementations
written twice in one language, `cross` for the rules written once per side.

```bash
jscpd fixtures/semantic-demo --semantic --semantic-scope same
# Found 2 clones.

jscpd fixtures/semantic-demo --semantic --semantic-scope cross
# Found 8 clones.
```

Scanning the two halves as two paths with `--skip-local` asks the same
question as `cross`: what did we write twice, once on each side?

```bash
jscpd --semantic --skip-local fixtures/semantic-demo/backend fixtures/semantic-demo/frontend
# Found 8 clones.
```

A stricter threshold keeps the closest pairs:

```bash
jscpd fixtures/semantic-demo --semantic --semantic-threshold 0.75
# Found 5 clones.
```

For an agent, `-r ai` prints one line per pair:

```bash
jscpd fixtures/semantic-demo --semantic -r ai
# backend/src/ feeds.rs:10-22 ~ text.rs:6-27 [~0.87 semantic]
```

The statistics table counts the first fragment of each pair as duplicated
lines, as it does for any clone. `--kind semantic` keeps only these clones,
and `-r json` marks them `"kind": "semantic"` with a `similarity`.

## An embeddings API instead

`--semantic-url` sends the functions to a server that speaks the OpenAI
embeddings API instead of running the model in jscpd: Ollama, LM Studio,
llama.cpp's `llama-server --embedding`, text-embeddings-inference or a hosted
API. Ollama serves the same model under another name and gives the same
pairs:

```bash
ollama pull unclemusclez/jina-embeddings-v2-base-code
jscpd fixtures/semantic-demo --semantic --semantic-url http://localhost:11434/v1
# Found 10 clones.
```

A key, when the API needs one, is read from `JSCPD_SEMANTIC_API_KEY` only,
and goes only to a URL given with `--semantic-url` or to a server on this
machine. A hosted API's URL therefore goes on the command line, and the
config file keeps the other settings:

```json
{
  "semantic": {
    "enabled": true,
    "provider": "http",
    "model": "jina-code-embeddings-0.5b",
    "dimensions": 256,
    "params": { "task": "code2code.query" }
  }
}
```

```bash
JSCPD_SEMANTIC_API_KEY=jina_… jscpd fixtures/semantic-demo --semantic-url https://api.jina.ai/v1
```

Similarity scales differ between models, so check the scores of a few known
pairs before trusting the default threshold with another model.

Functions are found in JavaScript, TypeScript, JSX, TSX, Vue, Svelte, Astro,
Python, Rust, Go, Java, Kotlin, C#, C, C++, PHP, Ruby, Scala and Swift files.
