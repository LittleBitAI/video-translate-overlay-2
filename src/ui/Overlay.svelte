<script lang="ts" module>
  /** The job's stage as the progress panel names it. */
  export const STAGE: Record<string, string> = {
    downloading: '내려받는 중',
    writing: 'write 중',
    translating: '번역 중',
  };

  export type Job = { stage: string; percent: number; cost_krw: number; failure_reason: string | null; video_key: string };

  /** Why a job stopped (P7's `failure_reason`). */
  export const STOPPED: Record<string, string> = {
    download_failed: '오디오를 내려받거나 읽지 못했습니다.',
    no_speech: '영상에서 말소리를 찾지 못했습니다.',
    key_rejected: 'Gemini가 개인키를 받아 주지 않았습니다. 설정에서 개인키를 확인해 주세요.',
    cost_limit: '비용 한도(1시간에 1,000원 안팎)를 넘어 번역을 멈췄습니다.',
    translate_failed: '번역하지 못했습니다. 잠시 뒤에 영상감지를 다시 눌러 주세요.',
  };
</script>

<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { findCue, type Cue } from '../overlay/index';
  import { request, won } from './api';
  import Link from './Link.svelte';

  type Props = {
    /** The page the video is on: the job is made and looked up by it. */
    pageUrl: string;
    title?: string;
    mediaUrl?: string | null;
    /** Where the stage's player is and how long it plays, in ms (0: not known yet). */
    timeMs?: number;
    durationMs?: number;
    /** Loads the player when it is not yet, and gives its length (0 when it cannot). */
    prepare?: () => Promise<number>;
    /** In the app 영상감지 is on the site's own overlay; this one only follows the job. */
    remote?: boolean;
    /** The page's job as P9 gave it (`null`: none yet); left out, P7 is asked. */
    initial?: Job | null;
  };
  let { pageUrl, title = '', mediaUrl = null, timeMs = 0, durationMs = 0, prepare, remote = false, initial }: Props = $props();

  const POLL_MS = 1_000;
  /** A player length this far from the lecture's is an ad's. */
  const AD_TOLERANCE_MS = 1_000;

  let job = $state<Job | null>(null);
  /** `checking`: the first P7 has not answered; `failed`: it answered with an error. */
  let look = $state<'checking' | 'done' | 'failed'>('checking');
  let starting = $state(false);
  let notice = $state<{ text: string; settings?: boolean } | null>(null);
  let cues = $state<Cue[]>([]);
  /** A ready job's subtitles (P8): asked for and not answered yet, or answered with an error. */
  let subs = $state<'none' | 'loading' | 'failed'>('none');
  let subsError = $state('');
  let mainMs = $state(0);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let gone = false;
  let generation = 0;
  let loadedJob = '';
  let deletionToken = $state<string | null>(null);

  const active = $derived(!!job && job.stage in STAGE);
  const ready = $derived((job?.stage === 'ready' || job?.stage === 'failed') && cues.length > 0);
  const ad = $derived(mainMs > 0 && durationMs > 0 && Math.abs(durationMs - mainMs) > AD_TOLERANCE_MS);
  const cue = $derived(ready && !ad ? findCue(cues, timeMs) : undefined);

  async function loadSubtitles(videoKey: string) {
    const mine = generation;
    subs = 'loading';
    const r = await request('GET', `/videos/${encodeURIComponent(videoKey)}/subtitles`);
    if (gone || mine !== generation) return;
    if (r.status === 200) {
      subs = 'none';
      cues = [...(r.body.cues as Cue[])].sort((a, b) => a.start_ms - b.start_ms);
      mainMs = r.body.duration_ms || 0;
      if (!cues.length) notice = { text: '자막으로 옮길 말이 없었습니다.' };
    } else if (job?.stage === 'failed') {
      subs = 'none';
    } else {
      subs = 'failed';
      subsError = r.body?.message ?? '저장해 둔 자막을 읽지 못했습니다. 영상감지 버튼을 다시 눌러 주세요.';
    }
  }

  /** Shows a job's state: its progress, or its subtitles once it is ready. */
  async function follow(j: Job | null) {
    look = 'done';
    job = j;
    const id = j ? `${(j as any).job_id ?? j.video_key}:${j.stage}` : '';
    if ((j?.stage === 'ready' || j?.stage === 'failed') && loadedJob !== id) {
      loadedJob = id;
      await loadSubtitles(j.video_key);
    }
    if (!gone && j) timer = setTimeout(poll, POLL_MS);
  }

  /** One look at the job (P7). */
  async function poll() {
    const mine = generation;
    timer = undefined;
    const r = await request('GET', '/jobs', { page_url: pageUrl });
    if (gone || mine !== generation) return;
    if (r.status === 404 || r.status === 410) {
      generation++; cues = []; loadedJob = ''; subs = 'none';
      deletionToken = r.body?.deletion_token ?? null;
      if (r.status === 410) notice = { text: r.body.message };
      return follow(null);
    }
    if (r.status === 200 || r.status === 404) return follow(r.status === 200 ? r.body : null);
    look = job ? 'done' : 'failed';
    if (job) timer = setTimeout(poll, POLL_MS);
  }

  async function detect() {
    if (deletionToken && !confirm('삭제한 영상의 자막을 새로 만듭니다. 추가 API 비용이 발생할 수 있습니다. 번역을 시작할까요?')) return;
    const mine = ++generation;
    if (timer) clearTimeout(timer);
    loadedJob = '';
    starting = true;
    notice = null;
    subs = 'none';
    const length = durationMs > 0 ? durationMs : ((await prepare?.()) ?? 0);
    if (gone || mine !== generation) return;
    const r = await request('POST', '/jobs', { page_url: pageUrl, media_url: mediaUrl, title, duration_ms: Math.round(length), ...(deletionToken ? {deletion_token:deletionToken} : {}) });
    if (gone || mine !== generation) return;
    starting = false;
    if (r.status === 200) {
      job = { stage: r.body.stage, percent: 0, cost_krw: 0, failure_reason: null, video_key: r.body.video_key };
      if (r.body.stage === 'ready') await poll();
      else timer = setTimeout(poll, POLL_MS);
    } else if (r.status === 410) {
      deletionToken = r.body?.deletion_token ?? null;
      notice = { text: r.body.message };
    } else if (r.status === 428) {
      notice = { text: '개인키가 없어 작업을 시작하지 않았습니다. 설정에서 Gemini 개인키를 넣어 주세요.', settings: true };
    } else if (r.status === 409) {
      notice = { text: r.body?.message ?? '다른 영상의 작업이 돌고 있습니다. 그 작업이 끝난 뒤에 다시 눌러 주세요.' };
    } else {
      notice = { text: r.body?.message ?? `작업을 시작하지 못했습니다 (${r.status}).` };
    }
  }

  onMount(() => {
    const invalidate = () => {
      generation++; job = null; cues = []; loadedJob = ''; starting = false;
      if (timer) clearTimeout(timer);
      poll();
    };
    const channel = new BroadcastChannel('vto-library');
    channel.onmessage = invalidate;
    window.addEventListener('vto-video-deleted', invalidate);
    initial === undefined ? poll() : follow(initial);
    return () => { channel.close(); window.removeEventListener('vto-video-deleted', invalidate); };
  });

  const box = 'rounded-lg bg-surface/90 p-2 text-sm';
  const label = 'text-sm text-ink/60';
  const value = 'font-semibold text-ink';
  /** The subtitle line: a dark band and a shadow, so it reads over a bright scene too. */
  const line = 'mx-auto mb-2 max-w-full rounded-md bg-black/60 px-3 py-1 text-center [text-shadow:0_0_4px_rgb(0_0_0/0.9),0_1px_2px_#000] sm:mb-[6%]';
  /** A button in a notice: 44 px high, so a thumb finds it. */
  const action = 'pointer-events-auto mt-2 inline-flex min-h-11 items-center rounded-lg px-4';
  onDestroy(() => {
    gone = true;
    generation++;
    if (timer) clearTimeout(timer);
  });
</script>

{#snippet progress(stage: string, percent: number, cost: number)}
  <div data-feature="job-progress" role="status" class="{box} w-full sm:w-56">
    <p class={label}>진행 상태</p>
    <p class="flex justify-between gap-2 text-base font-semibold"><span>{stage}</span><span>{percent}%</span></p>
    <div class="my-1 h-1.5 rounded-full bg-ink/25">
      <div class="h-1.5 rounded-full bg-primary" style:width="{Math.min(100, Math.max(0, percent))}%"></div>
    </div>
    <p><span class={label}>든 비용</span> <span class={value}>{won(cost)}</span></p>
  </div>
{/snippet}

<!-- The job's state: over the video on wider screens, under it on a phone, where the frame is too small to hold it. -->
{#snippet panel()}
  {#if look === 'checking'}
    <div class="h-6"></div>
  {:else if active && job}
    {@render progress(STAGE[job.stage], job.percent, job.cost_krw)}
  {:else if subs === 'loading' && job}
    <!-- The job is done; only its subtitles are on their way (there is no stage or percent left to show). -->
    <div data-feature="job-progress" role="status" class="{box} w-full sm:w-56">
      <p><span class={value}>자막 불러오는 중</span> <span class={label}>· {won(job.cost_krw)}</span></p>
      <div class="mt-1 h-1.5 overflow-hidden rounded-full bg-ink/25"><div class="vto-indeterminate h-1.5 w-1/3 rounded-full bg-primary"></div></div>
    </div>
  {:else if ready && job?.stage === 'ready'}
    <p data-feature="job-progress" role="status" class="w-fit rounded-lg bg-surface/90 px-2 py-1 text-sm text-ink/80">
      자막 준비됨 · {won(job.cost_krw)}
    </p>
  {:else if subs === 'failed' || notice || look === 'failed' || job?.stage === 'failed'}
    <!-- The reason; trying again is 영상감지, or the settings when the key is missing. -->
    <div data-feature="job-progress" role="alert" class="{box} pointer-events-auto w-full border border-accent sm:w-auto">
      {#if subs === 'failed'}
        <p class="text-accent">{subsError}</p>
      {:else if notice}
        <p class="text-accent">{notice.text}</p>
        {#if notice.settings}
          <Link href="/preferences" class="{action} bg-primary font-semibold text-surface">설정으로 가기</Link>
        {/if}
      {:else if look === 'failed'}
        <p class="text-accent">진행 상태를 불러오지 못했습니다. 영상감지 버튼을 다시 눌러 주세요.</p>
      {:else if job}
        <p class="text-accent">작업이 멈췄습니다. {STOPPED[job.failure_reason ?? ''] ?? job.failure_reason ?? ''}</p>
      {/if}
    </div>
  {:else if look === 'done'}
    <!-- No job yet: where its progress and cost will show. -->
    <div data-feature="job-progress" role="status" class="{box} w-full sm:w-40">
      <p><span class={label}>진행 상태</span> <span class={value}>작업 전</span></p>
      <p><span class={label}>든 비용</span> <span class={value}>{won(0)}</span></p>
    </div>
  {/if}
{/snippet}

<div class="pointer-events-none absolute inset-x-0 top-0 flex aspect-video flex-col justify-between gap-2 p-2">
  <div class="flex items-start justify-end gap-2 sm:justify-between">
    <div class="hidden min-w-0 sm:block sm:max-w-[70%]">{@render panel()}</div>

    {#if remote}
      {#if !active && !ready && look !== 'checking'}
        <p class="max-w-[50%] rounded-lg bg-surface/90 px-2 py-1 text-sm">영상 창의 영상감지 버튼을 누르세요</p>
      {/if}
    {:else}
      <!-- Always on the overlay: off while a job runs or its subtitles load; on a ready video it is a quiet outline, since it only shows the saved subtitles again (R25). -->
      <button
        type="button"
        data-feature="detect-video"
        class="pointer-events-auto min-h-11 shrink-0 rounded-lg px-4 py-2 text-base font-semibold disabled:bg-ink/15 disabled:text-ink/50 disabled:shadow-none {ready
          ? 'border border-ink/30 bg-surface/80 text-ink'
          : 'bg-primary text-surface shadow'}"
        disabled={starting || look === 'checking' || active || subs === 'loading'}
        onclick={detect}>{starting ? '확인하는 중' : '영상감지'}</button
      >
    {/if}
  </div>

  {#if cue}
    <p data-feature="watch-subtitles" class="{line} text-base text-ink sm:text-lg lg:text-2xl">
      {cue.text_ko}
    </p>
  {:else if !ready && look === 'done' && subs === 'none'}
    <!-- Where the subtitles will be; they come only once the whole job is done (R11). -->
    <p data-feature="watch-subtitles" class="{line} text-sm text-ink/70">
      {active ? '자막은 작업이 다 끝나면 여기에 뜹니다' : '한국어 자막이 여기에 뜹니다'}
    </p>
  {/if}
</div>
<div class="mt-2 sm:hidden">{@render panel()}</div>
