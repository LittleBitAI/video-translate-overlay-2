<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { inApp, request } from './api';

  type Account = {
    generation: number;
    state: 'unsupported' | 'loading' | 'signed_out' | 'signing_in' | 'signed_in' | 'expired' | 'unavailable';
    email: string | null;
    error: string | null;
    sync_ready: boolean;
  };

  const native = inApp();
  const outline = 'min-h-11 rounded-lg border border-ink/40 px-4 py-2 text-base disabled:opacity-50';
  let account = $state<Account | null>(null);
  let loading = $state(native);
  let sending = $state(false);
  let failure = $state('');
  let needsReload = $state(false);
  let confirming = $state(false);
  let generation = 0;
  let polling = false;

  function valid(value: any): value is Account {
    return Number.isSafeInteger(value?.generation) && value.generation >= 0
      && ['unsupported', 'loading', 'signed_out', 'signing_in', 'signed_in', 'expired', 'unavailable'].includes(value.state)
      && (value.email === null || typeof value.email === 'string')
      && (value.error === null || typeof value.error === 'string') && typeof value.sync_ready === 'boolean';
  }

  async function load() {
    const screen = ++generation;
    polling = false;
    sending = false;
    account = null;
    failure = '';
    needsReload = false;
    confirming = false;
    loading = true;
    const result = await request('GET', '/account');
    if (screen !== generation) return;
    loading = false;
    if (result.status !== 200 || !valid(result.body)) {
      failure = 'Google 계정 상태를 불러오지 못했습니다. 다시 시도해 주세요.';
      needsReload = true;
      return;
    }
    account = result.body;
  }

  async function poll() {
    if (polling || sending || !account || !['loading', 'signing_in'].includes(account.state) || needsReload) return;
    const screen = generation;
    const owner = account.generation;
    polling = true;
    const result = await request('GET', '/account');
    if (screen !== generation) return;
    polling = false;
    if (result.status !== 200 || !valid(result.body)) {
      failure = 'Google 계정 상태를 확인하지 못했습니다. 다시 불러와 주세요.';
      needsReload = true;
      return;
    }
    if (result.body.generation !== owner) {
      failure = '다른 화면에서 계정 상태가 바뀌었습니다. 다시 불러와 주세요.';
      needsReload = true;
      return;
    }
    account = result.body;
  }

  async function change(action: 'sign-in' | 'sign-out') {
    if (!account || sending || needsReload) return;
    const owner = account.generation;
    const screen = ++generation;
    polling = false;
    sending = true;
    failure = '';
    confirming = false;
    const result = await request('POST', `/account/${action}`, { generation: owner });
    if (screen !== generation) return;
    sending = false;
    if (result.status !== 200 || !valid(result.body)) {
      failure = result.body?.message ?? 'Google 계정을 변경하지 못했습니다. 다시 불러와 주세요.';
      needsReload = true;
      return;
    }
    // This operation owns the new account generation; older reads cannot publish into it.
    account = result.body;
  }

  onMount(() => {
    if (!native) return;
    // Arm the status watcher before any sign-in can start its native worker.
    const timer = window.setInterval(poll, 500);
    load();
    return () => window.clearInterval(timer);
  });
  onDestroy(() => generation++);
</script>

<section class="rounded-lg bg-ink/5 p-4 sm:p-6" aria-label="Google 앱 계정" aria-busy={loading || sending}>
  <h2 class="text-lg font-semibold">Google 앱 계정</h2>
  <p class="mt-1 text-sm text-ink/70">기기간 연결에 사용할 계정입니다. 유튜브 로그인과는 별개이며, Gemini 개인키와 쿠키를 동기화하지 않습니다.</p>
  {#if !native || account?.state === 'unsupported'}
    <p class="mt-2 text-sm text-ink/70">Windows 또는 Android 앱에서 Google 계정을 연결해 주세요.</p>
  {:else if loading}
    <p class="mt-2 text-sm text-ink/70" role="status">Google 계정 확인 중</p>
  {:else}
    {#if account?.state === 'signed_in'}
      <p class="mt-2 break-words text-base" role="status">연결한 Google 계정: <bdi>{account.email}</bdi></p>
      {#if !account.sync_ready}
        <p class="mt-1 text-sm text-ink/70">Google 계정은 연결됐습니다. 기기간 동기화는 아직 연결되지 않았습니다.</p>
      {/if}
    {:else if account?.state === 'signing_in'}
      <p class="mt-2 text-base" role="status">Google 화면에서 로그인을 완료해 주세요.</p>
    {:else if account?.state === 'loading'}
      <p class="mt-2 text-sm text-ink/70" role="status">저장한 Google 계정을 확인하는 중입니다.</p>
    {:else if account?.state === 'expired'}
      <p class="mt-2 text-sm text-accent" role="status">Google 로그인이 만료됐습니다. 다시 로그인해 주세요.</p>
    {:else if account?.state === 'unavailable' || account?.error}
      <p class="mt-2 text-sm text-accent" role="status">Google 로그인을 완료하지 못했습니다. 계정과 네트워크를 확인한 뒤 다시 시도해 주세요.</p>
    {:else if account}
      <p class="mt-2 text-sm text-ink/70" role="status">연결한 Google 계정이 없습니다.</p>
    {/if}

    {#if failure}
      <p class="mt-2 text-sm text-accent" role="alert">{failure}</p>
    {/if}
    {#if needsReload}
      <button type="button" class="{outline} mt-4" disabled={sending} onclick={load}>계정 상태 다시 불러오기</button>
    {:else if account && !['loading', 'unsupported'].includes(account.state)}
      {#if account.state === 'signing_in'}
        <button type="button" class="{outline} mt-4" disabled={sending} onclick={() => change('sign-out')}>Google 로그인 취소</button>
      {:else if account.state !== 'signed_in'}
        <button type="button" class="{outline} mt-4" disabled={sending} onclick={() => change('sign-in')}>Google 계정으로 로그인</button>
      {:else if confirming}
        <div class="mt-4 flex flex-wrap items-center gap-2 rounded-lg border border-accent/60 p-2" role="alertdialog" aria-label="Google 계정 로그아웃 확인">
          <p class="min-w-0 flex-1 text-sm">이 계정의 연결을 중지할까요? 저장한 데이터와 유튜브 로그인은 유지됩니다.</p>
          <button type="button" class={outline} disabled={sending} onclick={() => change('sign-out')}>Google 로그아웃 확인</button>
          <button type="button" class={outline} disabled={sending} onclick={() => confirming = false}>그만두기</button>
        </div>
      {:else}
        <button type="button" class="{outline} mt-4" disabled={sending} onclick={() => confirming = true}>Google 계정 로그아웃</button>
      {/if}
    {/if}
  {/if}
</section>
