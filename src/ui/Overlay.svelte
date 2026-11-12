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
  let mainMs = $state(0);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let gone = false;

  const active = $derived(!!job && job.stage in STAGE);
  const ready = $derived(job?.stage === 'ready' && cues.length > 0);
  const ad = $derived(mainMs > 0 && durationMs > 0 && Math.abs(durationMs - mainMs) > AD_TOLERANCE_MS);
  const cue = $derived(ready && !ad ? findCue(cues, timeMs) : undefined);

  async function loadSubtitles(videoKey: string) {
    const r = await request('GET', `/videos/${encodeURIComponent(videoKey)}/subtitles`);
    if (gone) return;
    if (r.status === 200) {
      cues = [...(r.body.cues as Cue[])].sort((a, b) => a.start_ms - b.start_ms);
      mainMs = r.body.duration_ms || 0;
      if (!cues.length) notice = { text: '자막으로 옮길 말이 없었습니다.' };
    } else {
      notice = { text: r.body?.message ?? '자막을 받지 못했습니다.' };
    }
  }

  /** Shows a job's state: its progress, or its subtitles once it is ready. */
  async function follow(j: Job | null) {
    look = 'done';
    job = j;
    if (j?.stage === 'ready') await loadSubtitles(j.video_key);
    else if (j && j.stage in STAGE) timer = setTimeout(poll, POLL_MS);
  }

  /** One look at the job (P7). */
  async function poll() {
    timer = undefined;
    const r = await request('GET', '/jobs', { page_url: pageUrl });
    if (gone) return;
    if (r.status === 200 || r.status === 404) return follow(r.status === 200 ? r.body : null);
    look = job ? 'done' : 'failed';
    if (job) timer = setTimeout(poll, POLL_MS);
  }

  async function detect() {
    starting = true;
    notice = null;
    const length = durationMs > 0 ? durationMs : ((await prepare?.()) ?? 0);
    if (gone) return;
    const r = await request('POST', '/jobs', { page_url: pageUrl, media_url: mediaUrl, title, duration_ms: Math.round(length) });
    if (gone) return;
    starting = false;
    if (r.status === 200) {
      job = { stage: r.body.stage, percent: 0, cost_krw: 0, failure_reason: null, video_key: r.body.video_key };
      if (r.body.stage === 'ready') await poll();
      else timer = setTimeout(poll, POLL_MS);
    } else if (r.status === 428) {
      notice = { text: '개인키가 없어 작업을 시작하지 않았습니다. 설정에서 Gemini 개인키를 넣어 주세요.', settings: true };
    } else if (r.status === 409) {
      notice = { text: r.body?.message ?? '다른 영상의 작업이 돌고 있습니다. 그 작업이 끝난 뒤에 다시 눌러 주세요.' };
    } else {
      notice = { text: r.body?.message ?? `작업을 시작하지 못했습니다 (${r.status}).` };
    }
  }

  onMount(() => (initial === undefined ? poll() : follow(initial)));
  onDestroy(() => {
    gone = true;
    if (timer) clearTimeout(timer);
  });
</script>

<div class="pointer-events-none absolute inset-0 flex flex-col justify-between gap-2 p-2">
  <div class="flex items-start justify-between gap-2">
    {#if look === 'checking'}
      <div class="h-6"></div>
    {:else if active && job}
      <div data-feature="job-progress" role="status" class="w-48 max-w-[60%] rounded-lg bg-surface/90 p-2 text-sm">
        <p class="flex justify-between gap-2"><span>진행 상태 <b>{STAGE[job.stage]}</b></span><span>{job.percent}%</span></p>
        <div class="my-1 h-1 rounded-full bg-ink/20">
          <div class="h-1 rounded-full bg-primary" style:width="{Math.min(100, Math.max(0, job.percent))}%"></div>
        </div>
        <p>든 비용 {won(job.cost_krw)}</p>
      </div>
    {:else if ready && job}
      <p data-feature="job-progress" role="status" class="rounded-lg bg-surface/90 px-2 py-1 text-sm text-accent">
        자막 준비됨 · {won(job.cost_krw)}
      </p>
    {:else if notice || look === 'failed' || job?.stage === 'failed'}
      <div data-feature="job-progress" role="status" class="pointer-events-auto max-w-[70%] rounded-lg bg-surface/90 p-2 text-sm">
        {#if notice}
          <p class="text-accent">{notice.text}</p>
          {#if notice.settings}
            <Link href="/preferences" class="mt-2 inline-block rounded-md bg-primary px-2 py-1 font-semibold text-surface">설정으로 가기</Link>
          {/if}
        {:else if look === 'failed'}
          <p class="text-accent">진행 상태를 불러오지 못했습니다.</p>
          <button type="button" class="mt-2 rounded-md border border-ink/40 px-2 py-1" onclick={() => ((look = 'checking'), poll())}>다시 불러오기</button>
        {:else if job}
          <p class="text-accent">작업이 멈췄습니다. {STOPPED[job.failure_reason ?? ''] ?? job.failure_reason ?? ''}</p>
        {/if}
      </div>
    {:else}
      <div></div>
    {/if}

    {#if remote}
      {#if !active && !ready && look !== 'checking'}
        <p class="max-w-[50%] rounded-lg bg-surface/90 px-2 py-1 text-sm">영상 창의 영상감지 버튼을 누르세요</p>
      {/if}
    {:else if !active && !ready}
      <button
        type="button"
        data-feature="detect-video"
        class="pointer-events-auto shrink-0 rounded-lg bg-primary px-4 py-2 text-base font-semibold text-surface shadow disabled:opacity-60"
        disabled={starting || look === 'checking'}
        onclick={detect}>{starting ? '확인하는 중' : '영상감지'}</button
      >
    {/if}
  </div>

  {#if cue}
    <p data-feature="watch-subtitles" class="mx-auto mb-[8%] max-w-full rounded bg-black/80 px-2 py-1 text-center text-base text-ink sm:text-lg lg:text-2xl">
      {cue.text_ko}
    </p>
  {/if}
</div>
