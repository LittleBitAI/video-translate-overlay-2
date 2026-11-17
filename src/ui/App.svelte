<script lang="ts">
  import { router } from './router.svelte';
  import Preferences from './Preferences.svelte';
  import Watch from './Watch.svelte';
  import Library from './Library.svelte';
  import Updates from './Updates.svelte';
</script>

<Updates />

<!-- Below 768 px the shared menu is a fixed bottom bar; this keeps the end of a screen above it. -->
<div class="max-md:pb-12">
  {#if router.path === '/library'}
    <Library />
  {:else if router.path === '/preferences'}
    <Preferences />
  {:else}
    <Watch />
  {/if}
</div>
