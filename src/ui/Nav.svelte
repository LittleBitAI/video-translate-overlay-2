<!-- The shared menu of the three screens, in the same place on each: tabs at the left of
     the top bar from 768 px, a bottom tab bar below that. A screen's own top-bar content
     (the address field on `/`) goes in `children`. -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Link from './Link.svelte';
  import { router } from './router.svelte';

  let { children }: { children?: Snippet } = $props();
  const tabs = [['/', '영상'], ['/library', '보관함'], ['/preferences', '설정']] as const;
</script>

<header class="border-b border-ink/10 bg-surface {children ? '' : 'max-md:border-0'}">
  <div class="flex items-center gap-4 md:px-4">
    <nav aria-label="주 메뉴" class="flex max-md:fixed max-md:inset-x-0 max-md:bottom-0 max-md:z-30 max-md:border-t max-md:border-ink/15 max-md:bg-surface max-md:pb-[env(safe-area-inset-bottom)]">
      {#each tabs as [href, label]}
        {@const current = router.path === href}
        <Link {href} current={current}
          class="inline-flex min-h-11 items-center justify-center border-primary px-4 text-base max-md:flex-1 max-md:border-t-2 md:min-h-15 md:border-b-2 {current ? 'font-semibold text-ink' : 'border-transparent text-ink/70 hover:text-ink'}">{label}</Link>
      {/each}
    </nav>
    {#if children}<div class="min-w-0 flex-1">{@render children()}</div>{/if}
  </div>
</header>
