# Reporting guide

Aggregation note 0: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 1: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 2: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 3: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 4: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 5: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 6: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 7: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 8: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 9: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 10: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 11: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 12: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 13: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 14: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 15: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 16: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation note 17: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.

```ts
export function buildReport(rows: Row[], limit: number): Report {
  const sorted = rows.slice().sort((a, b) => b.total - a.total);
  const top = sorted.slice(0, limit);
  const rest = sorted.slice(limit);
  const total = sorted.reduce((sum, row) => sum + row.total, 0);
  return { top, others: rest.length, total, generated: Date.now() };
}
```

Aggregation part two note 0: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 1: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 2: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 3: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 4: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 5: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 6: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 7: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 8: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 9: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 10: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 11: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 12: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 13: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 14: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 15: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 16: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part two note 17: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.

```ts
export function mergeReports(left: Report, right: Report): Report {
  const top = left.top.concat(right.top).slice(0, left.top.length);
  const others = left.others + right.others;
  const total = left.total + right.total;
  return { top, others, total, generated: Date.now() };
}
```

Aggregation part three note 0: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part three note 1: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part three note 2: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part three note 3: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part three note 4: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
Aggregation part three note 5: this paragraph is unique to this guide and is here only to put distance between the two code blocks below.
