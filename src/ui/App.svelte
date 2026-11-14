<script lang="ts">
  import { router } from './router.svelte';
  import Preferences from './Preferences.svelte';
  import Watch from './Watch.svelte';
  import Library from './Library.svelte';
  import Updates from './Updates.svelte';
</script>

<Updates />

{#if router.path === '/library'}
  <Library />
{:else if router.path === '/preferences'}
  <Preferences />
{:else}
  <Watch />
{/if}
