<script lang="ts">
  import type { Snippet } from 'svelte';
  import { go } from './router.svelte';

  let { href, class: cls = '', current = false, children }: { href: string; class?: string; current?: boolean; children: Snippet } = $props();

  function follow(e: MouseEvent) {
    if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    go(href);
  }
</script>

<a {href} class={cls} aria-current={current ? 'page' : undefined} onclick={follow}>{@render children()}</a>
