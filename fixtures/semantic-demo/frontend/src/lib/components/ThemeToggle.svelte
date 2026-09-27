<script lang="ts">
  import { onMount } from 'svelte';

  type Theme = 'light' | 'dark';
  const STORAGE_KEY = 'theme';

  let theme = $state<Theme>('light');

  function preferredTheme(): Theme {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === 'light' || stored === 'dark') {
      return stored;
    }
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    return prefersDark ? 'dark' : 'light';
  }

  function apply(next: Theme) {
    theme = next;
    document.documentElement.dataset.theme = next;
    localStorage.setItem(STORAGE_KEY, next);
    const meta = document.querySelector('meta[name="theme-color"]');
    meta?.setAttribute('content', next === 'dark' ? '#111318' : '#ffffff');
  }

  onMount(() => apply(preferredTheme()));
</script>

<button
  aria-label={theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme'}
  onclick={() => apply(theme === 'dark' ? 'light' : 'dark')}
>
  {theme === 'dark' ? '☀' : '☾'}
</button>
