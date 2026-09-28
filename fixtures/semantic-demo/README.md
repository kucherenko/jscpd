# Semantic clones (Type-4, experimental)

A semantic clone is code that does the same job as other code but uses
other names, another algorithm or another language. It shows up when a
project has two halves and the frontend repeats a rule the backend enforces,
and when two people in one codebase each solve the same problem their own
way. Token matching finds neither case. `--semantic` (config key `semantic`)
embeds every function with a code embedding model and reports two functions
as a clone of kind `semantic` when each is the other's closest match and
their cosine similarity reaches `--semantic-threshold`. A pair within one
language needs a higher similarity, `--semantic-same-threshold`. Both
default to the values jscpd calibrated for the model, 0.4125 and 0.6375 for
the default model, CodeRankEmbed.

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

The model runs inside jscpd. jscpd downloads it once into the user cache
directory (548 MB from huggingface.co) and checks it against its pinned
SHA-256:

```bash
jscpd --semantic-download
```

Run the commands from the repository root. They use the default
thresholds unless they set one. The first scan embeds 38 functions, which
takes a few seconds on a laptop. jscpd caches the vectors, so later runs
embed only what changed.

| Directory | What it holds | Default scan | `--semantic` |
|-----------|---------------|--------------|--------------|
| `backend/` + `frontend/` | 8 rules written in both Rust and Svelte, 2 features written twice in one language, code with no counterpart | 0 clones | 8 semantic clones, 10 with `--semantic-same-threshold 0.55` |

## Across languages

| Rule | Rust, `backend/src/` | Svelte, `frontend/src/lib/components/` | Similarity |
|------|----------------------|----------------------------------------|------------|
| Page links with gaps | `pagination.rs` `page_links` | `Pager.svelte` `visiblePages` | 0.49 |
| Card number checksum | `payments.rs` `is_valid_card_number` | `PaymentForm.svelte` `cardLooksValid` | 0.69 |
| Cart totals | `pricing.rs` `cart_totals` | `CartSummary.svelte` `computeTotals` | 0.73 |
| Title to slug | `feeds.rs` `feed_item_slug` | `ArticleCard.svelte` `toSlug` | 0.55 |
| Article preview | `text.rs` `excerpt` | `ArticleCard.svelte` `preview` | 0.54 |
| "3 days ago" | `time.rs` `relative_time` | `RelativeTime.svelte` `ago` | 0.48 |
| E-mail check | `validation.rs` `validate_email` | `SignupForm.svelte` `checkEmail` | 0.62 |
| Password strength | `validation.rs` `password_strength` | `SignupForm.svelte` `rate` | 0.67 |

No pair shares a name, and several share no structure either. The Rust
checksum walks the digits with an iterator and doubles every other one; the
Svelte version runs a `while` loop over a lookup table. `relative_time`
chains `match` guards; `ago` loops over a table of units. `validate_sku` in
`validation.rs` is shaped like `validate_email` but checks something else,
and it pairs with nothing. The backend makes slugs twice, and `toSlug` pairs
with the closer of the two, `feed_item_slug`.

## Within one language

| Feature | First | Second | Similarity |
|---------|-------|--------|------------|
| Title to slug | `backend/src/text.rs` `slugify`: a character loop | `backend/src/feeds.rs` `feed_item_slug`: split, filter, join | 0.62 |
| E-mail check | `SignupForm.svelte` `checkEmail`: step by step | `frontend/src/lib/validators.ts` `isEmail`: one regex | 0.60 |

The bar for two functions of one language is higher than the 0.4125 a pair
across languages needs, because code in one language resembles itself
whatever it does. With CodeRankEmbed it is 0.6375, and both pairs score just
under it, so the default scan leaves them out. The closest pair of unrelated
functions within one language in this demo scores 0.50, so a bar of 0.55
reports both duplicates and nothing else:

```bash
jscpd fixtures/semantic-demo --semantic --semantic-same-threshold 0.55
# Found 10 clones.
```

`checkEmail` then pairs with `validate_email` across languages and with
`isEmail` within one. jina-embeddings-v2-base-code, the other model jscpd
runs, reports both pairs at its own default bars:

```bash
jscpd --semantic-download --semantic-model jina-embeddings-v2-base-code
jscpd fixtures/semantic-demo --semantic --semantic-model jina-embeddings-v2-base-code
# Found 10 clones.
```

## Commands

```bash
jscpd fixtures/semantic-demo
# Found 0 clones.

jscpd fixtures/semantic-demo --semantic
# Semantic clones (experimental): embedding 38 functions with nomic-ai/CodeRankEmbed on this machine
# Clone found (rust, semantic ~0.55)
#  - backend/src/feeds.rs [10:1 - 22:2] (13 lines, 58 tokens)
#    frontend/src/lib/components/ArticleCard.svelte:typescript [7:3 - 16:4]
# Clone found (rust, semantic ~0.49)
#  - backend/src/pagination.rs [14:1 - 36:2] (23 lines, 172 tokens)
#    frontend/src/lib/components/Pager.svelte:typescript [12:3 - 30:4]
# ... six more pairs ...
# Found 8 clones.
```

`--semantic-scope` keeps one kind of pair: `same` for the implementations
written twice in one language, `cross` for the rules written once per side.
The default bars leave out the two same-language pairs, as described above:

```bash
jscpd fixtures/semantic-demo --semantic --semantic-scope same
# Found 0 clones.

jscpd fixtures/semantic-demo --semantic --semantic-scope same --semantic-same-threshold 0.55
# Found 2 clones.

jscpd fixtures/semantic-demo --semantic --semantic-scope cross
# Found 8 clones.
```

Scanning the two halves as two paths with `--skip-local` gives the same
pairs as `cross`, the code written once on each side:

```bash
jscpd --semantic --skip-local fixtures/semantic-demo/backend fixtures/semantic-demo/frontend
# Found 8 clones.
```

A stricter threshold keeps the closest pairs. It raises the bar within one
language by the model's gap, 0.225 for CodeRankEmbed, to 0.825 here, and
keeps the four pairs across languages that score 0.6 or more:

```bash
jscpd fixtures/semantic-demo --semantic --semantic-threshold 0.6
# Found 4 clones.
```

For an agent, `-r ai` prints one line per pair:

```bash
jscpd fixtures/semantic-demo --semantic -r ai
# backend/src/feeds.rs:10-22 ~ frontend/src/lib/components/ArticleCard.svelte:typescript:7-16 [~0.55 semantic]
```

The statistics table counts the first fragment of each pair as duplicated
lines, as it does for any clone. `--kind semantic` keeps only these clones,
and `-r json` marks them `"kind": "semantic"` with a `similarity`.

## The vector cache

After the first scan every vector comes from the cache, and the first line
of the output says so. `--semantic-rebuild-cache` embeds all 38 functions
again and replaces the cached vectors with the new ones; the run after it
reads them from the cache again:

```bash
jscpd fixtures/semantic-demo --semantic
# Semantic clones (experimental): 38 functions, all embeddings cached (nomic-ai/CodeRankEmbed)
# Found 8 clones.

jscpd fixtures/semantic-demo --semantic --semantic-rebuild-cache
# Semantic clones (experimental): embedding 38 functions with nomic-ai/CodeRankEmbed on this machine, rebuilding the cache
# Found 8 clones.
```

The flag does nothing without `--semantic`, and nothing when the config file
turns the cache off with `"cache": false`. jscpd warns in both cases.

You rarely need the flag. After a change to the body of `cart_totals` in
`backend/src/pricing.rs`, the next scan embeds that one function:

```
# Semantic clones (experimental): embedding 1 of 38 functions with nomic-ai/CodeRankEmbed on this machine, the rest cached
```

A scan of `fixtures/semantic-demo` and a scan of `fixtures/semantic-demo/backend`
keep their vectors in two separate folders of the cache. The vectors of
functions that changed or were deleted stay in the file until they make up
more than a quarter of it. The next scan that embeds something then rewrites
the file with only the vectors it used.

## An embeddings API instead

`--semantic-url` sends the functions to a server that speaks the OpenAI
embeddings API instead of running the model in jscpd: Ollama, LM Studio,
llama.cpp's `llama-server --embedding`, text-embeddings-inference or a hosted
API. Ollama has no copy of CodeRankEmbed, so the default model of an API is
jina-embeddings-v2-base-code under its Ollama name, and it gives the same 10
pairs as jscpd's own copy:

```bash
ollama pull unclemusclez/jina-embeddings-v2-base-code
jscpd fixtures/semantic-demo --semantic --semantic-url http://localhost:11434/v1
# Found 10 clones.
```

`--semantic-model` picks another model, and jscpd takes its thresholds from
the list that `jscpd --semantic-models` prints. Qwen3-Embedding-0.6B finds
seven of the eight rules, all but the article preview, and no pair within
one language:

```bash
ollama pull qwen3-embedding:0.6b
jscpd fixtures/semantic-demo --semantic --semantic-url http://localhost:11434/v1 --semantic-model qwen3-embedding:0.6b
# Found 7 clones.
```

When the API needs a key, jscpd reads it only from `JSCPD_SEMANTIC_API_KEY`
and sends it only to a URL given with `--semantic-url` or to a server on
this machine. So a hosted API's URL goes on the command line, and the config
file keeps the other settings:

```json
{
  "semantic": {
    "enabled": true,
    "provider": "http",
    "model": "text-embedding-3-small",
    "dimensions": 512
  }
}
```

```bash
JSCPD_SEMANTIC_API_KEY=sk-… jscpd fixtures/semantic-demo --semantic-url https://api.openai.com/v1
```

`dimensions` asks OpenAI for shorter vectors. OpenAI's text-embedding-3
models support it, and shorter vectors take less room in the cache.

Similarity scales differ between models. OpenAI's models are not in the
list that `jscpd --semantic-models` prints, so jscpd uses 0.6 and 0.75 for
them and warns that these thresholds are not calibrated. The same goes for
any other model outside that list. Check the scores of a few known pairs and
set both thresholds.

jscpd finds functions in JavaScript, TypeScript, JSX, TSX, Vue, Svelte,
Astro, Python, Rust, Go, Java, Kotlin, C#, C, C++, PHP, Ruby, Scala and Swift
files.
