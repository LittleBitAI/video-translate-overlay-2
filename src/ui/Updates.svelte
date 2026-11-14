<script lang="ts">
  import { onMount } from 'svelte';
  import { inApp, request } from './api';

  type Update = { version: string; summary: string; download_url: string };
  let update = $state<Update | null>(null);
  let checking = $state(false);
  let message = $state('');
  let failed = $state(false);
  let acting = $state(false);
  let alive = true;
  let queuedManual = false;
  let dismissedVersion = '';

  function valid(value: any, platform: string): value is Update {
    if (!value || !/^\d+\.\d+\.\d+$/.test(value.version) || typeof value.summary !== 'string' || value.summary.length > 1200) return false;
    const suffix = platform === 'android-arm64' ? 'arm64.apk' : platform === 'windows-x64' ? 'windows-x64-setup.exe' : '';
    return !!suffix && value.download_url === `https://github.com/LittleBitAI/video-translate-overlay-2/releases/download/v${value.version}/video-translate-overlay-${value.version}-${suffix}`;
  }

  async function check(manual = false) {
    if (checking) { queuedManual ||= manual; return; }
    checking = true;
    if (manual) { message = '업데이트 확인 중'; failed = false; }
    const r = await request('POST', '/updates/check', { manual });
    if (!alive) return;
    checking = false;
    if (r.status === 200 && typeof r.body?.installed_version === 'string') {
      update = valid(r.body.update, r.body.platform) && (manual || r.body.update.version !== dismissedVersion) ? r.body.update : null;
      if (manual) {
        failed = false;
        message = r.body.platform === 'unsupported' ? '이 기기에서는 업데이트를 지원하지 않습니다.' : update ? '' : `현재 버전 ${r.body.installed_version}입니다. 사용할 수 있는 새 업데이트가 없습니다.`;
      }
    } else if (manual) {
      failed = true;
      message = '업데이트를 확인하지 못했습니다. 인터넷 연결을 확인하고 다시 시도해 주세요.';
    }
    if (queuedManual) { queuedManual = false; void check(true); }
  }

  async function dismiss() {
    if (!update) return;
    const version = update.version;
    acting = true;
    const r = await request('POST', '/updates/dismiss', { version });
    if (!alive) return;
    acting = false;
    if (r.status === 200) {
      dismissedVersion = version;
      if (update?.version === version) { update = null; message = ''; }
    }
    else if (r.status !== 200) { failed = true; message = '업데이트 알림을 닫지 못했습니다. 다시 시도해 주세요.'; }
  }

  async function download(event: MouseEvent) {
    if (!inApp() || !update) return;
    event.preventDefault();
    if (acting) return;
    acting = true;
    const r = await request('POST', '/updates/open', { version: update.version });
    if (!alive) return;
    acting = false;
    if (r.status !== 200) { failed = true; message = '내려받기 링크를 열지 못했습니다. 업데이트를 다시 확인해 주세요.'; }
  }

  onMount(() => {
    alive = true;
    const automatic = () => { if (inApp() && document.visibilityState === 'visible') void check(); };
    const manual = () => void check(true);
    window.addEventListener('focus', automatic);
    window.addEventListener('vto-foreground', automatic);
    document.addEventListener('visibilitychange', automatic);
    window.addEventListener('vto-check-updates', manual);
    automatic();
    return () => {
      alive = false;
      window.removeEventListener('focus', automatic);
      window.removeEventListener('vto-foreground', automatic);
      document.removeEventListener('visibilitychange', automatic);
      window.removeEventListener('vto-check-updates', manual);
    };
  });
</script>

{#if update || message}
  <aside aria-label="앱 업데이트" aria-live="polite" class="mx-auto my-2 max-w-2xl rounded-lg border border-primary/50 bg-surface p-4 text-ink">
    {#if update}
      <h2 class="text-lg font-semibold">새 버전 {update.version}</h2>
      <p class="mt-2 whitespace-pre-line break-words text-sm">{update.summary}</p>
      <div class="mt-4 flex flex-wrap gap-2">
        <a class="inline-flex min-h-11 items-center rounded-lg bg-primary px-4 py-2 font-semibold text-surface" href={update.download_url} target="_blank" rel="noopener noreferrer" onclick={download} aria-disabled={acting}>업데이트 내려받기</a>
        <button type="button" class="min-h-11 rounded-lg border border-ink/40 px-4 py-2 disabled:opacity-50" disabled={acting} onclick={dismiss}>이 버전 알림 닫기</button>
      </div>
      <p class="mt-2 text-sm text-ink/70">브라우저에서 내려받은 뒤 직접 설치합니다. 기존 앱을 삭제하지 마세요.</p>
    {/if}
    {#if message}<p role="status" class="mt-2 text-sm {failed ? 'text-accent' : 'text-ink/70'}">{message}</p>{/if}
  </aside>
{/if}
