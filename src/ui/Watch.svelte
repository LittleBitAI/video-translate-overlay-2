<!-- S1 `/`: open a video by its address, then 영상감지 on the overlay over it,
     its progress and cost while the job runs, and its subtitles once it is ready. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { inApp, request } from './api';
  import Link from './Link.svelte';
  import type { Job } from './Overlay.svelte';
  import Stage from './Stage.svelte';

  /** P9: what the app last opened (`null`: nothing yet), and its job when P9 gave it. */
  let opened = $state<string | null>(null);
  let openedJob = $state<Job | null | undefined>(undefined);
  let screen = $state<'loading' | 'error' | 'ready'>('loading');
  let address = $state('');
  let opening = $state(false);
  let addressError = $state('');
  /** The app's ready notice for a video, while this screen is open (the OS one is shown too). */
  let readyNotice = $state<{ text: string; url: string } | null>(null);

  /** Google's account screen, shown before the first YouTube video: where it goes on to. */
  const gateNext = $derived.by(() => {
    try {
      const u = new URL(opened ?? '');
      return u.hostname === 'accounts.google.com' ? (u.searchParams.get('continue') ?? '') : null;
    } catch {
      return null;
    }
  });

  /** P9; `quiet` keeps what is shown until it answers. */
  async function load(quiet = false) {
    if (!quiet) screen = 'loading';
    const r = await request('GET', '/browser');
    if (r.status !== 200) {
      screen = 'error';
      return;
    }
    opened = r.body.url ?? null;
    openedJob = r.body.job ?? null;
    if (opened && !address) address = gateNext ?? opened;
    screen = 'ready';
  }

  async function open(url: string) {
    opening = true;
    addressError = '';
    const r = await request('POST', '/browser/open', { url });
    opening = false;
    if (r.status === 200) {
      // P9 again, for the new page's job along with it.
      await load(true);
    } else if (r.status === 400) {
      addressError = '열 수 없는 주소입니다. http:// 나 https:// 로 시작하는 영상 주소를 넣어 주세요.';
    } else {
      addressError = r.body?.message ?? `주소를 열지 못했습니다 (${r.status}).`;
    }
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    const url = address.trim();
    if (!url) {
      addressError = '영상 주소를 넣어 주세요.';
      return;
    }
    open(url);
  }

  onMount(() => {
    document.title = '영상 번역 오버레이';
    // The app calls this when a job's subtitles are ready.
    (window as any).vtoReady = (n: { text: string; url: string }) => (readyNotice = n);
    load();
    return () => delete (window as any).vtoReady;
  });
</script>

<header class="border-b border-ink/10 bg-surface">
  <form class="mx-auto flex max-w-5xl items-center gap-2 p-2 sm:p-4" onsubmit={submit} novalidate>
    <label class="sr-only" for="address">영상 주소</label>
    <input
      id="address"
      data-feature="open-site"
      class="min-h-11 min-w-0 flex-1 rounded-lg border border-ink/30 bg-ink/5 px-4 py-2 text-base placeholder:text-ink/50"
      type="text"
      inputmode="url"
      autocomplete="url"
      placeholder="영상 주소를 넣으세요"
      aria-describedby="address-error"
      aria-invalid={addressError ? 'true' : undefined}
      bind:value={address}
    />
    <button
      data-feature="open-site"
      class="min-h-11 shrink-0 rounded-lg bg-primary px-4 py-2 text-base font-semibold text-surface disabled:opacity-60"
      disabled={opening}>{opening ? '여는 중' : '열기'}</button
    >
    <Link href="/preferences" class="inline-flex min-h-11 min-w-11 shrink-0 items-center justify-center rounded-lg border border-ink/30 px-4 text-base">설정</Link>
  </form>
  {#if addressError}
    <p id="address-error" role="alert" class="mx-auto max-w-5xl px-2 pb-2 text-sm text-accent sm:px-4">{addressError}</p>
  {/if}
</header>

<main class="mx-auto max-w-5xl p-2 sm:p-4">
  {#if readyNotice}
    <div role="status" class="mb-4 flex flex-wrap items-center gap-2 rounded-lg border border-accent/60 p-2 text-sm">
      <p class="min-w-0 flex-1 break-words">{readyNotice.text}</p>
      <button type="button" class="rounded-md bg-primary px-2 py-1 font-semibold text-surface" onclick={() => (open(readyNotice!.url), (readyNotice = null))}>
        영상 열기
      </button>
    </div>
  {/if}

  {#if screen === 'loading'}
    <div aria-busy="true" aria-label="불러오는 중" class="relative aspect-video w-full overflow-hidden rounded-lg bg-ink/10">
      <div class="absolute inset-0 flex flex-col justify-between p-2">
        <div class="flex items-start justify-between gap-2">
          <div data-feature="job-progress" class="w-48 max-w-[60%] rounded-lg bg-surface/90 p-2 text-sm">
            <p class="flex justify-between gap-2"><span>진행 상태</span><span class="h-4 w-8 animate-pulse rounded bg-ink/20"></span></p>
            <div class="my-1 h-1 rounded-full bg-ink/20"></div>
            <p class="flex items-center gap-2">든 비용 <span class="h-4 w-12 animate-pulse rounded bg-ink/20"></span></p>
          </div>
          <button type="button" data-feature="detect-video" disabled class="rounded-lg bg-primary px-4 py-2 text-base font-semibold text-surface opacity-60">영상감지</button>
        </div>
      </div>
    </div>
  {:else if screen === 'error'}
    <div role="alert" class="rounded-lg border border-accent/60 p-4">
      <p class="text-base text-accent">화면을 불러오지 못했습니다. 앱이 켜져 있는지 확인하고 다시 불러와 주세요.</p>
      <button type="button" class="mt-4 rounded-lg border border-ink/40 px-4 py-2 text-base" onclick={() => load()}>다시 불러오기</button>
    </div>
  {:else if gateNext !== null}
    <section class="rounded-lg bg-ink/5 p-4 sm:p-6">
      <h1 class="text-lg font-semibold">유튜브 계정 화면</h1>
      <p class="mt-2 text-base text-ink/80">
        처음 유튜브를 열 때는 구글의 로그인 화면이 먼저 뜹니다. 로그인은 구글의 화면에서 하고, 이 앱은 비밀번호를 받지 않습니다.
      </p>
      {#if inApp()}
        <p class="mt-2 text-base text-ink/80">영상 창에 구글의 로그인 화면이 열렸습니다. 로그인하면 그 계정으로 영상이 열립니다.</p>
      {/if}
      <div class="mt-4 flex flex-wrap gap-2">
        {#if !inApp()}
          <a class="rounded-lg bg-primary px-4 py-2 text-base font-semibold text-surface" href={opened} target="_blank" rel="noopener noreferrer">구글 로그인 화면 열기</a>
        {/if}
        {#if gateNext}
          <button type="button" class="rounded-lg border border-ink/40 px-4 py-2 text-base" onclick={() => open(gateNext)}>유튜브 영상으로 가기</button>
        {/if}
      </div>
    </section>
  {:else if opened}
    {#key opened}
      <Stage url={opened} job={openedJob} />
    {/key}
  {:else}
    <div class="flex aspect-video w-full items-center justify-center rounded-lg border border-dashed border-ink/30 p-6 text-center">
      <p class="text-base text-ink/80">영상 주소를 넣으면 여기에서 열립니다</p>
    </div>
  {/if}
</main>
