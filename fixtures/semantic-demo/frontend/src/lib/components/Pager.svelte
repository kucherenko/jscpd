<script lang="ts">
  type Item = number | 'gap';

  let {
    page,
    pageCount,
    spread = 2,
    onchange
  }: { page: number; pageCount: number; spread?: number; onchange: (page: number) => void } =
    $props();

  function visiblePages(active: number, count: number, around: number): Item[] {
    if (count <= 0) return [];
    const current = Math.min(Math.max(active, 1), count);
    const start = Math.max(current - around, 1);
    const end = Math.min(current + around, count);
    const items: Item[] = [];
    if (start > 1) {
      items.push(1);
      if (start > 2) items.push('gap');
    }
    for (let p = start; p <= end; p++) {
      items.push(p);
    }
    if (end < count) {
      if (end < count - 1) items.push('gap');
      items.push(count);
    }
    return items;
  }

  const items = $derived(visiblePages(page, pageCount, spread));
</script>

<nav aria-label="Pagination">
  {#each items as item, i (i)}
    {#if item === 'gap'}
      <span class="gap">…</span>
    {:else}
      <button aria-current={item === page ? 'page' : undefined} onclick={() => onchange(item)}>
        {item}
      </button>
    {/if}
  {/each}
</nav>
