<template>
  <ul>
    <li class="export-0">{{ rows[0].label }}</li>
    <li class="export-1">{{ rows[1].label }}</li>
    <li class="export-2">{{ rows[2].label }}</li>
    <li class="export-3">{{ rows[3].label }}</li>
    <li class="export-4">{{ rows[4].label }}</li>
    <li class="export-5">{{ rows[5].label }}</li>
    <li class="export-6">{{ rows[6].label }}</li>
    <li class="export-7">{{ rows[7].label }}</li>
    <li class="export-8">{{ rows[8].label }}</li>
    <li class="export-9">{{ rows[9].label }}</li>
    <li class="export-10">{{ rows[10].label }}</li>
    <li class="export-11">{{ rows[11].label }}</li>
    <li class="export-12">{{ rows[12].label }}</li>
    <li class="export-13">{{ rows[13].label }}</li>
    <li class="export-14">{{ rows[14].label }}</li>
    <li class="export-15">{{ rows[15].label }}</li>
    <li class="export-16">{{ rows[16].label }}</li>
    <li class="export-17">{{ rows[17].label }}</li>
    <li class="export-18">{{ rows[18].label }}</li>
    <li class="export-19">{{ rows[19].label }}</li>
    <li class="export-20">{{ rows[20].label }}</li>
    <li class="export-21">{{ rows[21].label }}</li>
    <li class="export-22">{{ rows[22].label }}</li>
    <li class="export-23">{{ rows[23].label }}</li>
    <li class="export-24">{{ rows[24].label }}</li>
    <li class="export-25">{{ rows[25].label }}</li>
    <li class="export-26">{{ rows[26].label }}</li>
    <li class="export-27">{{ rows[27].label }}</li>
  </ul>
</template>

<script lang="ts">
export function buildReport(rows: Row[], limit: number): Report {
  const sorted = rows.slice().sort((a, b) => b.total - a.total);
  const top = sorted.slice(0, limit);
  const rest = sorted.slice(limit);
  const total = sorted.reduce((sum, row) => sum + row.total, 0);
  return { top, others: rest.length, total, generated: Date.now() };
}

export function mergeReports(left: Report, right: Report): Report {
  const top = left.top.concat(right.top).slice(0, left.top.length);
  const others = left.others + right.others;
  const total = left.total + right.total;
  return { top, others, total, generated: Date.now() };
}
</script>
