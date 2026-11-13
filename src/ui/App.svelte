<script lang="ts">
  import { router } from './router.svelte';
  import Preferences from './Preferences.svelte';
  import Watch from './Watch.svelte';
</script>

{#if router.path === '/preferences'}
  <Preferences />
{:else}
  <Watch />
{/if}
