<!-- S2 `/preferences`: put in or delete the Gemini key, and sign out of YouTube. -->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { inApp, request } from './api';
  import Nav from './Nav.svelte';
  import GoogleAccount from './GoogleAccount.svelte';

  type Settings = { key_set: boolean; key_masked: string | null; translation_model: string };

  let screen = $state<'loading' | 'error' | 'ready'>('loading');
  let s = $state<Settings>({ key_set: false, key_masked: null, translation_model: '' });

  let key = $state('');
  let saving = $state(false);
  let keyError = $state('');
  let keySaved = $state(false);

  let asking = $state<'delete' | 'logout' | null>(null);
  let busy = $state(false);
  let done = $state('');
  let failed = $state('');
  let connecting = $state(false);
  let connectionGeneration = 0;
  let privacyOpening = $state(false);
  let privacyError = $state('');
  let privacyGeneration = 0;
  const youtubeSignIn = 'https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fwww.youtube.com%2F';

  async function connectYoutube() {
    const generation = ++connectionGeneration;
    connecting = true;
    done = '';
    failed = '';
    const r = await request('POST', '/browser/open', { url: youtubeSignIn });
    if (generation !== connectionGeneration) return;
    connecting = false;
    if (r.status === 200) done = '유튜브 계정 화면을 열었습니다. 로그인하면 유튜브로 돌아갑니다.';
    else failed = r.body?.message ?? '유튜브 계정 화면을 열지 못했습니다. 다시 시도해 주세요.';
  }

  onDestroy(() => { connectionGeneration++; privacyGeneration++; });

  async function openPrivacy() {
    if (privacyOpening) return;
    const generation = privacyGeneration;
    privacyOpening = true;
    privacyError = '';
    const result = await request('POST', '/privacy/open');
    if (generation !== privacyGeneration) return;
    privacyOpening = false;
    if (result.status !== 200) privacyError = '개인정보처리방침을 열지 못했습니다. 다시 시도해 주세요.';
  }

  async function load() {
    screen = 'loading';
    const r = await request('GET', '/settings');
    if (r.status !== 200) {
      screen = 'error';
      return;
    }
    s = r.body;
    screen = 'ready';
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    keyError = '';
    keySaved = false;
    if (!key.trim()) {
      keyError = '개인키를 넣어 주세요.';
      return;
    }
    saving = true;
    const r = await request('PUT', '/settings/api-key', { api_key: key });
    saving = false;
    if (r.status === 200) {
      s = { ...s, key_set: true, key_masked: r.body.key_masked };
      key = '';
      keySaved = true;
      if (screen === 'error') load();
    } else if (r.status === 400 || r.status === 401) {
      keyError = '쓸 수 없는 개인키입니다';
    } else if (r.status === 503 || r.status === 0) {
      keyError = '지금은 확인할 수 없습니다. 잠시 뒤에 다시 눌러 주세요';
    } else {
      keyError = r.body?.message ?? `개인키를 저장하지 못했습니다 (${r.status}).`;
    }
  }

  async function confirm() {
    const what = asking;
    busy = true;
    failed = '';
    const r = what === 'delete' ? await request('DELETE', '/settings/api-key') : await request('DELETE', '/browser/site-data');
    busy = false;
    asking = null;
    if (r.status !== 200) {
      failed = r.body?.message ?? `하지 못했습니다 (${r.status}).`;
      return;
    }
    if (what === 'delete') {
      s = { ...s, key_set: false, key_masked: null };
      keySaved = false;
      done = '개인키를 이 기기에서 지웠습니다.';
    } else {
      done = '유튜브에서 로그아웃했습니다. 다음에 유튜브를 열면 구글의 로그인 화면이 다시 뜹니다.';
    }
  }

  function ask(what: 'delete' | 'logout') {
    asking = what;
    done = '';
    failed = '';
  }

  onMount(() => {
    document.title = '설정 · 영상 번역 오버레이';
    load();
  });

  const section = 'settings-section min-w-0 max-lg:rounded-lg max-lg:bg-ink/5 max-lg:p-4 sm:max-lg:p-6';
  const outline = 'min-h-11 rounded-lg border border-ink/40 px-4 py-2 text-base disabled:opacity-50';
</script>

{#snippet confirmRow(what: 'delete' | 'logout', question: string, yes: string, feature: string)}
  <div role="alertdialog" aria-label={question} class="mt-2 flex flex-wrap items-center gap-2 rounded-lg border border-accent/60 p-2 md:col-span-full">
    <p class="min-w-0 flex-1 text-base">{question}</p>
    <button type="button" data-feature={feature} class="rounded-lg bg-accent px-4 py-2 text-base font-semibold text-surface disabled:opacity-60" disabled={busy} onclick={confirm}>
      {yes}
    </button>
    <button type="button" class={outline} disabled={busy} onclick={() => (asking = null)}>그만두기</button>
  </div>
{/snippet}

<Nav />

<main class="flex flex-col gap-6 p-2 sm:p-4 md:min-h-[calc(100dvh-61px)]">
  <h1 class="px-2 pt-2 text-2xl font-semibold sm:px-0">설정</h1>
  <div class="grid w-full max-w-[90rem] items-start gap-6 lg:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)]">
    <section class={section} aria-busy={screen === 'loading'}>
      <div>
        <h2 class="text-lg font-semibold">Gemini 개인키</h2>
        <p class="mt-1 text-sm text-ink/70">개인키는 이 기기에만 저장됩니다. 번역 비용은 개인키 주인에게 청구됩니다.</p>
      </div>
      <div class="max-w-2xl">
        {#if screen === 'error'}
          <!-- The saved settings did not load: its own box, apart from the field, whose place is for a key's own errors. -->
          <div role="alert" class="my-2 flex flex-wrap items-center gap-2 rounded-lg border border-accent p-2">
            <p class="min-w-0 flex-1 text-sm text-accent">설정을 불러오지 못했습니다. 잠시 뒤에 다시 눌러 주세요.</p>
            <button type="button" class="min-h-11 rounded-lg border border-accent px-4 py-2 text-base text-accent" onclick={load}>다시 불러오기</button>
          </div>
        {/if}

        <form class="mt-6 max-w-2xl" onsubmit={save} novalidate>
          <label for="api-key" class="sr-only">Gemini 개인키</label>
          <div class="mt-1 flex gap-2">
            <input
              id="api-key"
              data-feature="set-api-key"
              class="min-h-11 min-w-0 flex-1 rounded-lg border bg-ink/5 px-4 py-2 text-base placeholder:text-ink/50 {keyError ? 'border-accent' : 'border-ink/30'} disabled:opacity-60"
              type="password"
              autocomplete="off"
              spellcheck="false"
              placeholder="개인키 붙여 넣기"
              aria-describedby="key-message"
              aria-invalid={keyError ? 'true' : undefined}
              disabled={saving || screen === 'loading'}
              bind:value={key}
            />
            <button
              data-feature="set-api-key"
              class="inline-flex min-h-11 shrink-0 items-center justify-center gap-2 rounded-lg bg-primary px-4 py-2 text-base font-semibold text-surface sm:min-w-36 {saving ? 'cursor-wait opacity-80' : 'disabled:opacity-50'}"
              disabled={saving || screen === 'loading'}
            >
              {#if saving}
                <span class="size-4 animate-spin rounded-full border-2 border-surface border-t-transparent" aria-hidden="true"></span>확인하는 중
              {:else}
                저장
              {/if}
            </button>
          </div>
          <p id="key-message" role={keyError ? 'alert' : 'status'} class="mt-1 text-sm {keyError ? 'text-accent' : 'text-ink/70'}">
            {#if keyError}{keyError}{:else if keySaved}Gemini가 확인한 개인키를 저장했습니다.{/if}
          </p>
        </form>

        {#if screen === 'loading'}
          <dl class="mt-2 grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-2 text-sm">
            <dt>저장된 개인키</dt>
            <dd class="h-4 w-32 animate-pulse rounded bg-ink/20"></dd>
            <dt>번역 모듈</dt>
            <dd class="h-4 w-40 animate-pulse rounded bg-ink/20"></dd>
          </dl>
        {:else if screen === 'error'}
          <!-- What is saved is not known: nothing is claimed about it. -->
        {:else if s.key_set}
          <dl class="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
            <dt>저장된 개인키</dt>
            <dd class="font-mono break-all">{s.key_masked} <span class="font-sans text-ink/60">끝 네 글자만 보입니다</span></dd>
            <dt>번역 모듈</dt>
            <dd>{s.translation_model}</dd>
          </dl>
        {:else}
          <p class="mt-2 flex items-center gap-2 text-sm text-ink/70"><svg aria-hidden="true" viewBox="0 0 16 16" class="size-4 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="8" cy="8" r="7" /><path d="M8 7v4.5M8 4.5v.01" stroke-linecap="round" /></svg>개인키를 넣어야 영상감지 버튼이 작업을 시작합니다</p>
          <dl class="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
            <dt>저장된 개인키</dt>
            <dd class="text-ink/60">없음</dd>
            <dt>번역 모듈</dt>
            <dd>{s.translation_model}</dd>
          </dl>
        {/if}

        <hr class="my-4 border-ink/15 md:hidden" />
        <div class="md:mt-4 md:grid md:grid-cols-[minmax(0,1fr)_auto] md:items-start md:gap-4">
          <p class="mb-4 text-sm md:mb-0 {s.key_set ? 'text-ink/70' : 'text-ink/40'}">지우면 이 기기에서 사라지고, 영상감지 버튼을 누를 때 개인키를 넣으라는 안내가 뜹니다.</p>
          {#if asking === 'delete'}
            {@render confirmRow('delete', '개인키를 지울까요?', '지우기', 'delete-api-key')}
          {:else}
            <button type="button" data-feature="delete-api-key" class={outline} disabled={!s.key_set || screen !== 'ready'} onclick={() => ask('delete')}>
              개인키 지우기
            </button>
          {/if}
        </div>
      </div>
    </section>
    <div class="min-w-0 space-y-6 lg:border-s lg:border-ink/10 lg:ps-6">
      <GoogleAccount />
      <section class="{section} lg:border-t lg:border-ink/10 lg:pt-6">
        <div>
          <h2 class="text-lg font-semibold">유튜브 로그인</h2>
          <p class="mt-1 text-sm text-ink/70">
            한 번 로그인하면 앱에 저장한 세션으로 다음 영상을 엽니다. 로그인은 구글 화면에서 진행하며, 앱은 비밀번호를 받거나 저장하지 않습니다.
          </p>
          <p class="mt-1 text-sm text-ink/70">휴대폰의 유튜브 앱과는 별도 로그인입니다. 구글이 앱 내 로그인을 차단하면 계정을 연결할 수 없습니다.</p>
        </div>
        <div class="mt-4 flex flex-wrap items-center gap-2">
          {#if inApp()}
            <button type="button" class="{outline}" disabled={connecting || busy || screen !== 'ready'} onclick={connectYoutube}>
              {connecting ? '계정 화면 여는 중' : '유튜브 계정 연결'}
            </button>
          {:else}
            <a class="{outline} inline-flex items-center" href={youtubeSignIn} target="_blank" rel="noopener noreferrer">유튜브 계정 연결</a>
          {/if}
          {#if asking === 'logout'}
            {@render confirmRow('logout', '유튜브에서 로그아웃할까요?', '로그아웃', 'youtube-logout')}
          {:else}
            <button type="button" data-feature="youtube-logout" class="{outline}" disabled={connecting || busy || screen !== 'ready'} onclick={() => ask('logout')}>
              유튜브 로그아웃
            </button>
          {/if}
        </div>
      </section>
    </div>
  </div>
  {#if done || failed}
    <p role="status" class="text-sm {failed ? 'text-accent' : 'text-ink/80'}">{failed || done}</p>
  {/if}
  <footer class="mt-auto flex w-full max-w-[90rem] flex-wrap items-center justify-between gap-x-6 gap-y-4 border-t border-ink/10 pt-4">
    <section aria-label="앱 업데이트" class="flex flex-wrap items-center gap-4">
      <div>
        <h2 class="text-sm font-semibold">앱 업데이트</h2>
        <p class="mt-1 text-sm text-ink/70">승인된 새 버전과 추가 기능을 확인합니다. 닫은 버전도 다시 볼 수 있습니다.</p>
      </div>
      <button type="button" class="inline-flex min-h-11 items-center px-2 text-sm text-ink/70 underline underline-offset-4 hover:text-ink" onclick={() => window.dispatchEvent(new Event('vto-check-updates'))}>업데이트 확인</button>
    </section>
    <div>
      {#if inApp()}
        <button type="button" class="inline-flex min-h-11 items-center text-sm text-ink/70 underline underline-offset-4 hover:text-ink" disabled={privacyOpening} onclick={openPrivacy}>개인정보처리방침</button>
      {:else}
        <a class="inline-flex min-h-11 items-center text-sm text-ink/70 underline underline-offset-4 hover:text-ink" href="https://littlebitai.github.io/video-translate-overlay-2/privacy.html" target="_blank" rel="noopener noreferrer">개인정보처리방침</a>
      {/if}
      {#if privacyError}<p class="mt-2 text-sm text-accent" role="alert">{privacyError}</p>{/if}
    </div>
  </footer>
</main>
