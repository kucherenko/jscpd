<script lang="ts">
  import type { Article } from '$lib/types';
  import RelativeTime from './RelativeTime.svelte';

  let { article, previewWords = 30 }: { article: Article; previewWords?: number } = $props();

  function toSlug(text: string): string {
    return text
      .normalize('NFKD')
      .replace(/[̀-ͯ]/g, '')
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 80)
      .replace(/-+$/, '');
  }

  function preview(text: string, limit: number): string {
    const words = text.trim().split(/\s+/);
    if (words.length <= limit) {
      return words.join(' ');
    }
    const shortened = words.slice(0, limit).join(' ').replace(/[,;:.\-]+$/, '');
    return `${shortened}…`;
  }

  const href = $derived(`/articles/${toSlug(article.title)}`);
</script>

<article>
  <h2><a {href}>{article.title}</a></h2>
  <p>{preview(article.body, previewWords)}</p>
  <footer>
    <RelativeTime date={new Date(article.publishedAt)} />
  </footer>
</article>
