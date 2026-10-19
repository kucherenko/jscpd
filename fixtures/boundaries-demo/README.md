# Boundaries

A boundary rule in `.jscpd.json` names code that must stay where it is: a
module, a folder, a file or a method. When a copy of that code shows up in a
place the rule keeps it out of, jscpd reports it and exits with code 1. A
copy here means copy-paste, a copy someone edited, or the same logic written
again in another language. Import linters such as dependency-cruiser or
ArchUnit check who imports whom; a copied function imports nothing, so they
don't see it.

This demo is a small shop that also gives loans. `backend/pricing/` holds the
discount and the sales tax in TypeScript, and `backend/loans/Interest.kt`
holds the monthly payment of a loan in Kotlin. The rule `money` says that
only the backend calculates money:

```json
{
  "name": "money",
  "code": ["backend/pricing", "backend/loans/Interest.kt#monthlyPayment"],
  "notIn": ["web/**"],
  "allow": ["web/e2e/**"],
  "thresholds": { "minTokens": 30, "similarity": 0.7 }
}
```

`code` protects the whole `backend/pricing` folder and one method of
`Interest.kt`. `notIn` keeps copies out of `web/`, `allow` lets the
end-to-end tests have them, and `thresholds` looks for smaller and looser
copies than a clone report does.

## Setup

The rules compare functions with the embedding model of `--semantic`, with
or without that flag. Download it once, 548 MB into the user cache folder:

```bash
jscpd --semantic-download
```

Without the model the rules still run and find less; see
[Without the model](#without-the-model).

Run the commands from the repository root. `--config` reads the demo's rules,
and their paths start in the demo folder, next to the config file.

| File | What it holds | Reported |
|------|---------------|----------|
| `web/src/checkout/discount.ts` | a copy of `applyDiscount` | yes, `exact` |
| `web/src/checkout/tax.ts` | `salesTax` with other names and a debug line | yes, `similar (ast) ~0.84` |
| `web/src/loan/payment.ts` | `monthlyPayment` of `Interest.kt` written again in TypeScript | yes, `semantic ~0.82` |
| `web/src/loan/rate.ts` | `formatRate` of `Interest.kt` in TypeScript | no, the rule protects `monthlyPayment` only |
| `web/e2e/discount.spec.ts` | the discount, for the total a test expects | no, `allow` takes `web/e2e/` |
| `backend/orders/order.ts` | the discount in another backend module | no, `notIn` is `web/` only |

## The check

```bash
jscpd --config fixtures/boundaries-demo/.jscpd.json fixtures/boundaries-demo
```

The clone report comes first, as usual: three exact clones of the discount.
After its table, the console reporter lists what the rule found:

```text
Boundaries:
money: 3 copies outside the boundary
  web/src/checkout/discount.ts:8-17 applyDiscount
    copy of backend/pricing/discount.ts:6-15 applyDiscount (exact, 10 lines)
  web/src/checkout/tax.ts:6-15 estimateTax
    looks like backend/pricing/tax.ts:5-13 salesTax (similar (ast) ~0.84)
  web/src/loan/payment.ts:4-14 monthlyPayment
    looks like backend/loans/Interest.kt:8-15 monthlyPayment (semantic ~0.82)
```

jscpd then writes this line to stderr and exits with code 1:

```text
ERROR: jscpd found 3 copies of protected code outside their boundaries (money: 3)
```

The copy of the discount starts on line 8 because the token clone starts at
the last `;` of `cartBadge`, the function before it.

`(cd fixtures/boundaries-demo && jscpd .)` finds the same three copies: jscpd
reads `.jscpd.json` from the working directory.

## Other reporters

`-r ai` adds one line per copy, the copy first:

```text
Boundaries:
money: web/src/checkout/discount.ts:8-17 ~ backend/pricing/discount.ts:6-15
money: web/src/checkout/tax.ts:6-15 ~ backend/pricing/tax.ts:5-13 [~0.84 ast]
money: web/src/loan/payment.ts:4-14 ~ backend/loans/Interest.kt:8-15 [~0.82 semantic]
3 copies outside boundaries
```

`-r json` adds a `boundaries` key to `jscpd-report.json` with every rule and
its violations. Each side names its file, its lines and the function it lies
in:

```json
{
  "kind": "semantic",
  "similarity": 0.82,
  "lines": 11,
  "tokens": 80,
  "copy": { "name": "web/src/loan/payment.ts", "start": 4, "end": 14, "function": "monthlyPayment" },
  "protected": { "name": "backend/loans/Interest.kt", "start": 8, "end": 15, "function": "monthlyPayment" }
}
```

The real report also has `startLoc` and `endLoc` on each side.

## Without the model

jscpd prints a warning and checks the rule without the model:

```text
Warning: boundaries: the model nomic-ai/CodeRankEmbed is not downloaded, so the rules look for copies without it and miss logic written again in another language. Run `jscpd --semantic-download` once (548 MB), or use an embeddings API with --semantic-url
```

The report then says `money: 2 copies outside the boundary`: the copy of the
discount and the tax with other names, as in [The check](#the-check). The
monthly payment written again in TypeScript takes the model to find, so it is
not there. The run still exits with code 1.

## A rule that needs no model

`no-semantic.jscpd.json` holds the same rule with one key more,
`"disable": ["semantic"]`. The rule then looks for copies and edited copies
only, needs no model and prints no warning:

```bash
jscpd --config fixtures/boundaries-demo/no-semantic.jscpd.json fixtures/boundaries-demo
```

The report says `money: 2 copies outside the boundary`, the same two copies
as without the model, and the run exits with code 1. `disable` takes the
names of `--kind`: `exact`, `renamed`, `gap`, `ast`, `similar` for both
`gap` and `ast`, and `semantic`.

## Whole directory

`jscpd fixtures/boundaries-demo` without `--config` reads the config of the
repository root, which has no rules: it reports the three exact clones and
exits with code 0.
