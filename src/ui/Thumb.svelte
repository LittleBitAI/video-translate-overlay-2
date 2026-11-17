<!-- A 16:9 preview. A YouTube video's key is its video ID, so its image comes from YouTube's
     image server; any other site, a missing video, or a failed load gets a tile with the
     site's host and a play mark. -->
<script lang="ts" module>
  export function host(pageUrl: string): string {
    try {
      return new URL(pageUrl).hostname.replace(/^www\.|^m\./, '');
    } catch {
      return '';
    }
  }
  export function youtubeImage(videoKey: string | undefined): string | null {
    return videoKey && /^[A-Za-z0-9_-]{11}$/.test(videoKey) ? `https://i.ytimg.com/vi/${videoKey}/mqdefault.jpg` : null;
  }
</script>

<script lang="ts">
  let { videoKey, pageUrl = '', small = false }: { videoKey?: string; pageUrl?: string; small?: boolean } = $props();
  let broken = $state(false);
  const src = $derived(youtubeImage(videoKey));
</script>

<span data-src={src} class="relative block aspect-video w-full overflow-hidden rounded-lg bg-ink/10">
  {#if src && !broken}
    <!-- YouTube answers an unknown ID with a 120 px grey image; that counts as a failed load. -->
    <img {src} alt="" loading="lazy" decoding="async" class="size-full object-cover"
      onerror={() => (broken = true)} onload={e => { if (e.currentTarget.naturalWidth <= 120) broken = true; }} />
  {:else}
    <span data-placeholder class="flex size-full flex-col items-center justify-center gap-1 p-2 text-ink/70">
      <svg aria-hidden="true" viewBox="0 0 24 24" class={small ? 'size-5' : 'size-8'} fill="currentColor"><path d="M8 5.5v13l11-6.5z" /></svg>
      {#if !small && pageUrl}<span class="max-w-full truncate text-sm">{host(pageUrl)}</span>{/if}
    </span>
  {/if}
</span>
