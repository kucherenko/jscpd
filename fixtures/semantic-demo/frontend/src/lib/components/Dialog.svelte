<script lang="ts">
  import type { Snippet } from 'svelte';

  let { open = $bindable(false), title, children }: { open?: boolean; title: string; children: Snippet } =
    $props();

  let panel: HTMLDivElement | undefined = $state();

  const FOCUSABLE = 'a[href], button:not([disabled]), input, select, textarea, [tabindex]:not([tabindex="-1"])';

  // Keep Tab inside the dialog and close it on Escape.
  function onKeydown(event: KeyboardEvent) {
    if (!open || !panel) return;
    if (event.key === 'Escape') {
      open = false;
      return;
    }
    if (event.key !== 'Tab') return;
    const focusable = Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE));
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="backdrop" role="presentation" onclick={() => (open = false)}></div>
  <div class="panel" role="dialog" aria-modal="true" aria-label={title} bind:this={panel}>
    <h2>{title}</h2>
    {@render children()}
  </div>
{/if}
