<script lang="ts">
  import type { Snippet } from 'svelte';
  import { go } from './router.svelte';

  let { href, class: cls = '', children }: { href: string; class?: string; children: Snippet } = $props();

  function follow(e: MouseEvent) {
    if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    go(href);
  }
</script>

<a {href} class={cls} onclick={follow}>{@render children()}</a>
