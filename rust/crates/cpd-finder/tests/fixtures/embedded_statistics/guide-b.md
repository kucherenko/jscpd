# Reporting guide

Intro paragraph 0: the guide explains the reporting pipeline in detail.
Intro paragraph 1: the guide explains the reporting pipeline in detail.
Intro paragraph 2: the guide explains the reporting pipeline in detail.
Intro paragraph 3: the guide explains the reporting pipeline in detail.
Intro paragraph 4: the guide explains the reporting pipeline in detail.
Intro paragraph 5: the guide explains the reporting pipeline in detail.
Intro paragraph 6: the guide explains the reporting pipeline in detail.
Intro paragraph 7: the guide explains the reporting pipeline in detail.
Intro paragraph 8: the guide explains the reporting pipeline in detail.
Intro paragraph 9: the guide explains the reporting pipeline in detail.
Intro paragraph 10: the guide explains the reporting pipeline in detail.
Intro paragraph 11: the guide explains the reporting pipeline in detail.

```ts
export function buildReport(rows: Row[], limit: number): Report {
  const sorted = rows.slice().sort((a, b) => b.total - a.total);
  const top = sorted.slice(0, limit);
  const rest = sorted.slice(limit);
  return {
    top,
    others: rest.length,
    total: sorted.reduce((sum, row) => sum + row.total, 0),
  };
}
```

Middle paragraph 0: the guide explains the reporting pipeline in detail.
Middle paragraph 1: the guide explains the reporting pipeline in detail.
Middle paragraph 2: the guide explains the reporting pipeline in detail.
Middle paragraph 3: the guide explains the reporting pipeline in detail.
Middle paragraph 4: the guide explains the reporting pipeline in detail.
Middle paragraph 5: the guide explains the reporting pipeline in detail.
Middle paragraph 6: the guide explains the reporting pipeline in detail.
Middle paragraph 7: the guide explains the reporting pipeline in detail.
Middle paragraph 8: the guide explains the reporting pipeline in detail.
Middle paragraph 9: the guide explains the reporting pipeline in detail.
Middle paragraph 10: the guide explains the reporting pipeline in detail.
Middle paragraph 11: the guide explains the reporting pipeline in detail.

```ts
export function mergeReports(left: Report, right: Report): Report {
  const rows = left.top.concat(right.top);
  const others = left.others + right.others;
  const total = left.total + right.total;
  return {
    top: rows,
    others,
    total,
  };
}
```

Outro paragraph 0: the guide explains the reporting pipeline in detail.
Outro paragraph 1: the guide explains the reporting pipeline in detail.
Outro paragraph 2: the guide explains the reporting pipeline in detail.
Outro paragraph 3: the guide explains the reporting pipeline in detail.
Outro paragraph 4: the guide explains the reporting pipeline in detail.
Outro paragraph 5: the guide explains the reporting pipeline in detail.
Outro paragraph 6: the guide explains the reporting pipeline in detail.
Outro paragraph 7: the guide explains the reporting pipeline in detail.
Outro paragraph 8: the guide explains the reporting pipeline in detail.
Outro paragraph 9: the guide explains the reporting pipeline in detail.
Outro paragraph 10: the guide explains the reporting pipeline in detail.
Outro paragraph 11: the guide explains the reporting pipeline in detail.
