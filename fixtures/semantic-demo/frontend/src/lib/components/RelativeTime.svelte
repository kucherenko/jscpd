<script lang="ts">
  import { onMount } from 'svelte';

  let { date }: { date: Date } = $props();
  let now = $state(new Date());

  const UNITS: [limitInSeconds: number, seconds: number, name: string][] = [
    [3600, 60, 'minute'],
    [86400, 3600, 'hour'],
    [604800, 86400, 'day'],
    [2592000, 604800, 'week'],
    [31536000, 2592000, 'month'],
    [Infinity, 31536000, 'year']
  ];

  function ago(when: Date, reference: Date): string {
    const elapsed = Math.max(0, Math.floor((reference.getTime() - when.getTime()) / 1000));
    if (elapsed < 45) return 'just now';
    if (elapsed >= 86400 && elapsed < 172800) return 'yesterday';
    for (const [limit, size, name] of UNITS) {
      if (elapsed < limit) {
        const count = Math.max(1, Math.floor(elapsed / size));
        return `${count} ${name}${count === 1 ? '' : 's'} ago`;
      }
    }
    return 'a long time ago';
  }

  onMount(() => {
    const timer = setInterval(() => (now = new Date()), 30_000);
    return () => clearInterval(timer);
  });
</script>

<time datetime={date.toISOString()} title={date.toLocaleString()}>{ago(date, now)}</time>
