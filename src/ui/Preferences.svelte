<!-- S2 `/preferences`: put in or delete the Gemini key, and sign out of YouTube. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { request } from './api';
  import Link from './Link.svelte';

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

  const card = 'rounded-lg bg-ink/5 p-4 sm:p-6';
  const outline = 'min-h-11 rounded-lg border border-ink/40 px-4 py-2 text-base disabled:opacity-50';
</script>

{#snippet confirmRow(what: 'delete' | 'logout', question: string, yes: string, feature: string)}
  <div role="alertdialog" aria-label={question} class="mt-2 flex flex-wrap items-center gap-2 rounded-lg border border-accent/60 p-2">
    <p class="min-w-0 flex-1 text-base">{question}</p>
    <button type="button" data-feature={feature} class="rounded-lg bg-accent px-4 py-2 text-base font-semibold text-surface disabled:opacity-60" disabled={busy} onclick={confirm}>
      {yes}
    </button>
    <button type="button" class={outline} disabled={busy} onclick={() => (asking = null)}>그만두기</button>
  </div>
{/snippet}

<header class="border-b border-ink/10">
  <div class="mx-auto flex max-w-2xl items-center gap-4 p-4">
    <Link href="/" class="inline-flex min-h-11 items-center text-base underline underline-offset-4">← 영상으로</Link>
    <h1 class="text-lg font-semibold">설정</h1>
  </div>
</header>

<main class="mx-auto flex max-w-2xl flex-col gap-4 p-2 sm:p-4">
  <section class={card} aria-busy={screen === 'loading'}>
    <h2 class="text-lg font-semibold">개인키</h2>
    <p class="mt-1 text-sm text-ink/70">개인키는 이 기기에만 저장됩니다. 번역 비용은 개인키 주인에게 청구됩니다.</p>

    <form class="mt-4" onsubmit={save} novalidate>
      <label for="api-key" class="text-sm">Gemini 개인키</label>
      <div class="mt-1 flex gap-2">
        <input
          id="api-key"
          data-feature="set-api-key"
          class="min-h-11 min-w-0 flex-1 rounded-lg border bg-ink/5 px-4 py-2 text-base placeholder:text-ink/50 {keyError ? 'border-accent' : 'border-ink/30'}"
          type="password"
          autocomplete="off"
          spellcheck="false"
          placeholder="개인키를 붙여 넣으세요"
          aria-describedby="key-message"
          aria-invalid={keyError ? 'true' : undefined}
          disabled={screen === 'loading'}
          bind:value={key}
        />
        <button
          data-feature="set-api-key"
          class="inline-flex min-h-11 min-w-36 shrink-0 items-center justify-center gap-2 rounded-lg bg-primary px-4 py-2 text-base font-semibold text-surface disabled:opacity-60"
          disabled={saving || screen === 'loading'}
        >
          {#if saving || screen === 'loading'}
            <span class="size-4 animate-spin rounded-full border-2 border-surface border-t-transparent" aria-hidden="true"></span>확인하는 중
          {:else}
            저장
          {/if}
        </button>
      </div>
      <p id="key-message" role={keyError || screen === 'error' ? 'alert' : 'status'} class="mt-1 text-sm {keyError || screen === 'error' ? 'text-accent' : 'text-ink/70'}">
        {#if keyError}{keyError}{:else if screen === 'error'}저장된 설정을 불러오지 못했습니다. 다시 불러와 주세요.{:else if keySaved}Gemini가 확인한 개인키를 저장했습니다.{/if}
      </p>
      {#if screen === 'error'}
        <button type="button" class="{outline} mt-2" onclick={load}>다시 불러오기</button>
      {/if}
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
      <p class="mt-2 text-sm text-accent">개인키를 넣어야 영상감지 버튼이 작업을 시작합니다</p>
      <dl class="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
        <dt>저장된 개인키</dt>
        <dd class="text-ink/60">없음</dd>
        <dt>번역 모듈</dt>
        <dd>{s.translation_model}</dd>
      </dl>
    {/if}

    <hr class="my-4 border-ink/15" />
    <p class="mb-4 text-sm text-ink/70">지우면 이 기기에서 사라지고, 영상감지 버튼을 누를 때 개인키를 넣으라는 안내가 뜹니다.</p>
    {#if asking === 'delete'}
      {@render confirmRow('delete', '개인키를 지울까요?', '지우기', 'delete-api-key')}
    {:else}
      <button type="button" data-feature="delete-api-key" class={outline} disabled={!s.key_set || screen !== 'ready'} onclick={() => ask('delete')}>
        개인키 지우기
      </button>
    {/if}
  </section>

  <section class={card}>
    <h2 class="text-lg font-semibold">유튜브 로그인</h2>
    <p class="mt-1 text-sm text-ink/70">
      유튜브 로그인은 구글의 화면에서 하고, 앱은 비밀번호를 받거나 저장하지 않습니다. 로그아웃하면 다음에 유튜브를 열 때 구글의 계정 화면이 다시 뜹니다.
    </p>
    {#if asking === 'logout'}
      {@render confirmRow('logout', '유튜브에서 로그아웃할까요?', '로그아웃', 'youtube-logout')}
    {:else}
      <button type="button" data-feature="youtube-logout" class="{outline} mt-4" disabled={screen !== 'ready'} onclick={() => ask('logout')}>
        유튜브 로그아웃
      </button>
    {/if}
  </section>

  {#if done || failed}
    <p role="status" class="text-sm {failed ? 'text-accent' : 'text-ink/80'}">{failed || done}</p>
  {/if}
</main>
