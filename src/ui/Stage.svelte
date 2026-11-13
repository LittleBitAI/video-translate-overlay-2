<script lang="ts" module>
  const MEDIA = /\.(mp4|m4v|m4a|mov|webm|mkv|mp3|aac|wav|ogg|oga|opus|m3u8)$/i;

  /** How the first screen shows a page: a media file in its own player, a YouTube video in its embed, or neither. */
  export function stageKind(url: string): { kind: 'video' } | { kind: 'youtube'; id: string } | { kind: 'page' } {
    let u: URL;
    try {
      u = new URL(url);
    } catch {
      return { kind: 'page' };
    }
    const host = u.hostname.replace(/^(www|m|music)\./, '');
    const id =
      host === 'youtu.be'
        ? u.pathname.slice(1)
        : host === 'youtube.com'
          ? (u.searchParams.get('v') ?? /^\/(?:shorts|embed|live)\/([^/]+)/.exec(u.pathname)?.[1] ?? '')
          : '';
    if (/^[\w-]{11}$/.test(id)) return { kind: 'youtube', id };
    return MEDIA.test(u.pathname) ? { kind: 'video' } : { kind: 'page' };
  }
</script>

<script lang="ts">
  import { onDestroy } from 'svelte';
  import { inApp } from './api';
  import Overlay, { type Job } from './Overlay.svelte';

  let { url, job }: { url: string; job?: Job | null } = $props();

  const shown = $derived(stageKind(url));
  let timeMs = $state(0);
  let durationMs = $state(0);
  let title = $state('');

  // ---- a media file
  let video = $state<HTMLVideoElement>();
  let ticker: ReturnType<typeof setInterval> | undefined;
  /** timeupdate comes about 4 times a second; while playing the time is read every 100 ms. */
  function track() {
    if (!video) return;
    timeMs = video.currentTime * 1000;
    if (Number.isFinite(video.duration)) durationMs = video.duration * 1000;
    if (!video.paused && !ticker) ticker = setInterval(track, 100);
    if (video.paused && ticker) {
      clearInterval(ticker);
      ticker = undefined;
    }
  }

  // ---- a YouTube embed, read through its postMessage API (`listening` / `infoDelivery`)
  let frame = $state<HTMLIFrameElement>();
  let embed = $state(false);
  let yt = { at: 0, ms: 0, playing: false };
  function listen() {
    frame?.contentWindow?.postMessage(JSON.stringify({ event: 'listening', id: 1, channel: 'widget' }), 'https://www.youtube-nocookie.com');
  }
  function fromYoutube(e: MessageEvent) {
    if (e.origin !== 'https://www.youtube-nocookie.com' || typeof e.data !== 'string') return;
    let m: any;
    try {
      m = JSON.parse(e.data);
    } catch {
      return;
    }
    const info = m?.info;
    if (!info || (m.event !== 'infoDelivery' && m.event !== 'initialDelivery')) return;
    if (typeof info.duration === 'number' && info.duration > 0) durationMs = info.duration * 1000;
    if (typeof info.videoData?.title === 'string') title = info.videoData.title;
    if (typeof info.playerState === 'number') yt.playing = info.playerState === 1;
    if (typeof info.currentTime === 'number') yt = { ...yt, at: performance.now(), ms: info.currentTime * 1000 };
  }
  addEventListener('message', fromYoutube);
  const ytTicker = setInterval(() => {
    if (shown.kind === 'youtube' && embed) timeMs = yt.playing ? yt.ms + (performance.now() - yt.at) : yt.ms;
  }, 100);

  /** 영상감지 before the player knows its length: load it and wait for the length (up to 15 s). */
  async function prepare(): Promise<number> {
    if (shown.kind === 'page') return 0;
    if (shown.kind === 'youtube') embed = true;
    for (let waited = 0; durationMs <= 0 && waited < 15_000; waited += 250) await new Promise((r) => setTimeout(r, 250));
    return durationMs;
  }

  onDestroy(() => {
    removeEventListener('message', fromYoutube);
    clearInterval(ytTicker);
    if (ticker) clearInterval(ticker);
  });
</script>

<!-- The overlay lies over the frame; on a phone its status panel goes under it. -->
<div class="relative">
  <div class="relative aspect-video w-full overflow-hidden rounded-lg bg-black">
    {#if inApp()}
      <div class="absolute inset-0 flex items-center justify-center p-6 text-center text-base text-ink/80">
        <p>영상은 앱의 영상 창에 열렸습니다. 그 영상 위에 영상감지 버튼과 자막이 뜹니다.</p>
      </div>
    {:else if shown.kind === 'video'}
      <!-- svelte-ignore a11y_media_has_caption: the Korean subtitles are drawn over it by the overlay -->
      <video
        bind:this={video}
        src={url}
        controls
        playsinline
        preload="metadata"
        class="absolute inset-0 h-full w-full"
        ontimeupdate={track}
        onplay={track}
        onpause={track}
        onseeked={track}
        onloadedmetadata={track}
        ondurationchange={track}
      ></video>
    {:else if shown.kind === 'youtube'}
      {#if embed}
        <iframe
          bind:this={frame}
          title="유튜브 영상"
          src="https://www.youtube-nocookie.com/embed/{shown.id}?enablejsapi=1&autoplay=1&playsinline=1&origin={encodeURIComponent(location.origin)}"
          allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
          class="absolute inset-0 h-full w-full border-0"
          onload={listen}
        ></iframe>
      {:else}
        <button type="button" class="absolute inset-0 flex items-center justify-center text-ink/90" onclick={() => (embed = true)}>
          <span class="rounded-full bg-surface/80 px-6 py-4 text-lg">▶ 유튜브 영상 재생</span>
        </button>
      {/if}
    {:else}
      <div class="absolute inset-0 flex items-center justify-center p-6 text-center text-base text-ink/80">
        <p>이 화면에서는 유튜브 영상과 영상 파일 주소만 재생합니다. 다른 사이트의 영상은 앱에서 열어 주세요.</p>
      </div>
    {/if}
  </div>

  <Overlay pageUrl={url} {title} mediaUrl={shown.kind === 'video' ? url : null} {timeMs} {durationMs} {prepare} remote={inApp()} initial={job} />
</div>
