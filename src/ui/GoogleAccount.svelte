<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { inApp, request } from './api';

  type Account = {
    generation: number;
    state: 'unsupported' | 'loading' | 'signed_out' | 'signing_in' | 'signed_in' | 'expired' | 'unavailable';
    email: string | null;
    error: string | null;
    sync_ready: boolean;
    connection?: { state: string; last_success: number | null };
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
  const connectionText: Record<string, string> = {
    listening:'PC에서 연결을 기다립니다. 휴대폰에서 같은 Google 계정으로 로그인하면 자동으로 연결합니다.',
    searching:'PC 앱을 켜고 같은 네트워크에 연결해 주세요. PC가 유선이고 휴대폰이 Wi-Fi여도 연결할 수 있습니다. 연결되지 않으면 Windows 방화벽의 비공개 네트워크 허용과 공유기의 게스트망·기기간 차단 설정을 확인해 주세요.',
    connected:'최근 동기화를 완료했습니다. 두 기기의 보관함을 계속 자동으로 확인합니다.',
    no_private_network:'연결할 로컬 네트워크가 없습니다. PC와 휴대폰을 같은 네트워크에 연결해 주세요.',
    discovery_unavailable:'주변 PC를 찾지 못했습니다. 같은 네트워크인지 확인하고, 공유기의 기기간 차단과 방화벽 설정을 확인해 주세요. 자동으로 다시 시도합니다.',
    listener_unavailable:'PC에서 기기 연결을 열지 못했습니다. 방화벽의 비공개 네트워크 허용과 네트워크 상태를 확인해 주세요. 자동으로 다시 시도합니다.',
    background_paused:'백그라운드 연결을 유지하지 못했습니다. 앱의 알림 권한과 백그라운드 설정을 확인하고 앱 화면으로 돌아와 주세요. 저장한 변경은 유지합니다.',
    authentication_required:'새 기기에 계정을 확인하려면 Google 로그인 갱신이 필요합니다. PC와 휴대폰에서 같은 계정으로 다시 로그인해 주세요. 저장한 보관함은 유지합니다.',
    interrupted:'기기 연결이 끊겼습니다. PC와 네트워크 상태를 확인해 주세요. 저장한 변경을 유지하며 자동으로 다시 시도합니다.',
    account_mismatch:'다른 Google 계정의 기기와는 연결하지 않았습니다. PC와 휴대폰의 Google 앱 계정을 확인해 주세요.',
    rejected:'유효한 기기 연결을 확인하지 못해 데이터를 보내지 않았습니다. 두 기기의 앱과 Google 앱 계정을 확인해 주세요. 자동으로 다시 시도합니다.',
    stopped:'기기간 연결을 중지했습니다. 앱을 다시 열면 연결을 확인합니다.',
  };

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
    if (polling || sending || !account || !['loading', 'signing_in', 'signed_in'].includes(account.state) || needsReload) return;
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
    const changed = (event: Event) => {
      if (sending || !account || (event as CustomEvent).detail === account.generation) return;
      generation++; account = null; confirming = false; polling = false;
      failure = '다른 화면에서 계정 상태가 바뀌었습니다. 다시 불러와 주세요.'; needsReload = true;
    };
    window.addEventListener('vto-account-changed', changed);
    load();
    return () => { window.clearInterval(timer); window.removeEventListener('vto-account-changed', changed); };
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
      {#if account.connection && connectionText[account.connection.state]}
        <p class="mt-2 text-sm text-ink/70" role="status">{connectionText[account.connection.state]}</p>
        {#if account.connection.state === 'authentication_required'}
          <button type="button" class="{outline} mt-4" disabled={sending || needsReload} onclick={() => change('sign-in')}>Google 로그인 갱신</button>
        {/if}
      {:else if !account.sync_ready}
        <p class="mt-1 text-sm text-ink/70">Google 계정은 연결됐습니다. 기기간 동기화는 아직 연결되지 않았습니다.</p>
      {/if}
    {:else if account?.state === 'signing_in'}
      <p class="mt-2 text-base" role="status">Google 화면에서 로그인을 완료해 주세요.</p>
    {:else if account?.state === 'loading'}
      <p class="mt-2 text-sm text-ink/70" role="status">저장한 Google 계정을 확인하는 중입니다.</p>
    {:else if account?.state === 'expired'}
      <p class="mt-2 text-sm text-accent" role="status">기기의 Google 연결이 만료됐습니다. 다시 로그인해 주세요. 이 계정의 저장한 보관함은 계속 열 수 있습니다.</p>
    {:else if account?.error === 'Google Desktop registration is missing or invalid'}
      <p class="mt-2 text-sm text-accent" role="status">앱 제공자가 Google 로그인 등록 정보를 수정해야 합니다. 수정된 앱에서 다시 시도해 주세요.</p>
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
