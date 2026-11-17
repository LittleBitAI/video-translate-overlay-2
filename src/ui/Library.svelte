<!-- S3 `/library`: people come back to reopen a translated lecture or to go through a topic
     playlist. Activating a playlist shows its videos at once: in the right pane beside a
     playlist rail from 768 px, directly under the tapped row below that. Reordering,
     renaming and deleting wait behind 편집; adding waits behind 추가. -->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { MediaQuery } from 'svelte/reactivity';
  import { request } from './api';
  import { go } from './router.svelte';
  import Nav from './Nav.svelte';
  import Thumb, { host } from './Thumb.svelte';

  type Video = { video_key: string; title: string; page_url: string; duration_ms?: number; saved: boolean; active: boolean; deleting: boolean; versions?: {id:string; selected:boolean}[] };
  type Playlist = { id: string; name: string; entries: string[] };
  let videos = $state<Video[]>([]);
  let playlists = $state<Playlist[]>([]);
  /** `''` is 전체 영상, otherwise a playlist id; `null` (phones only) leaves every row closed. */
  let selected = $state<string | null>(null);
  let search = $state('');
  let name = $state('');
  let rename = $state('');
  let url = $state('');
  let linkTitle = $state('');
  let editing = $state(false);
  let adding = $state(false);
  let screen = $state<'loading'|'ready'|'error'>('loading');
  let error = $state('');
  let busy = $state(false);
  let refreshPending = false;
  let confirmation = $state<{ kind: 'video'|'playlist'; id: string; title: string } | null>(null);
  let generation = 0;
  let channel: BroadcastChannel;
  let retired = false;
  let owner: number | undefined;
  let legacyAvailable = $state(false);
  let importing = $state(false);
  let accountEmail = $state<string | null>(null);
  let root = $state<HTMLElement>();
  let drag = $state<{ kind: 'playlist'|'entry'; key: string; over: string } | null>(null);

  const wide = new MediaQuery('min-width: 768px');
  /** What is shown: a wide window always shows something; a search on a phone opens 전체 영상. */
  const open = $derived(wide.current ? (selected ?? '') : (selected ?? (search ? '' : null)));
  const playlist = $derived(playlists.find(p => p.id === open));
  const byKey = $derived(new Map(videos.map(v => [v.video_key, v])));
  const members = (p: Playlist) => p.entries.map(k => byKey.get(k)).filter((v): v is Video => !!v);
  const listed = $derived(open === null ? [] : playlist ? members(playlist) : videos);
  const visible = $derived(listed.filter(v => `${v.title} ${v.page_url}`.toLocaleLowerCase().includes(search.toLocaleLowerCase())));
  const button = 'min-h-11 rounded-lg border border-ink/30 px-4 py-2 text-base disabled:opacity-40';
  const input = 'min-h-11 min-w-0 rounded-lg border border-ink/30 bg-ink/5 px-4 py-2 text-base';

  async function load() {
    refreshPending = false;
    const mine = ++generation;
    // A refresh keeps the shown library (and a phone's scroll position) until it answers.
    if (screen !== 'ready') screen = 'loading';
    const r = await request('GET', '/library/data');
    if (mine !== generation) return;
    if (r.status !== 200) { screen = 'error'; error = r.body?.message ?? '보관함을 불러오지 못했습니다.'; return; }
    videos = r.body.videos;
    playlists = r.body.playlists;
    owner = r.body.account_generation;
    accountEmail = typeof r.body.account_email === 'string' ? r.body.account_email : null;
    legacyAvailable = r.body.legacy_available === true;
    if (selected && !playlists.some(p => p.id === selected)) selected = null;
    screen = 'ready';
  }

  async function change(action: Record<string, unknown>) {
    if (busy) return;
    busy = true;
    error = '';
    const mine = ++generation;
    const known = new Set(playlists.map(p => p.id));
    const r = await request('POST', '/library/data', { ...action, account_generation: owner });
    if (action.action === 'delete_video' && (r.status === 200 || r.status >= 500)) {
      if (!retired) channel.postMessage(action.video_key);
    }
    if (mine !== generation) return;
    busy = false;
    if (r.status !== 200) {
      error = r.body?.message ?? '변경하지 못했습니다. 다시 시도해 주세요.';
      if (refreshPending || (action.action === 'delete_video' && r.status >= 500)) await load();
      return;
    }
    confirmation = null;
    name = ''; url = ''; linkTitle = '';
    await load();
    const made = action.action === 'create_playlist' && playlists.find(p => !known.has(p.id));
    if (made) choose(made.id);
  }

  async function openVideo(video: Video) {
    busy = true; error = '';
    const mine = ++generation;
    const r = await request('POST', '/browser/open', { url: video.page_url, account_generation: owner });
    if (mine !== generation) return;
    busy = false;
    if (r.status === 200) go('/');
    else {
      error = r.body?.message ?? '영상을 열지 못했습니다.';
      if (refreshPending) await load();
    }
  }

  /** Selects a playlist (`''`: 전체 영상); on a phone, tapping the open row closes it. */
  function choose(id: string) {
    selected = !wide.current && open === id ? null : id;
    rename = playlists.find(p => p.id === id)?.name ?? '';
    search = ''; url = ''; linkTitle = ''; confirmation = null; error = '';
  }

  /** Moves by any number of places in one request; the backend clamps at the ends. */
  function move(kind: 'playlist'|'entry', key: string, delta: number) {
    if (!delta) return;
    change(kind === 'playlist' ? {action:'move_playlist', playlist_id:key, delta} : {action:'move_entry', playlist_id:playlist!.id, video_key:key, delta});
  }
  const order = (kind: 'playlist'|'entry') => kind === 'playlist' ? playlists.map(p => p.id) : playlist?.entries ?? [];

  // Dragging a handle: the item whose centre is nearest the pointer is the drop place.
  function dragStart(e: PointerEvent, kind: 'playlist'|'entry', key: string) {
    if (busy) return;
    e.preventDefault();
    drag = { kind, key, over: key };
    const finish = (commit: boolean) => () => {
      removeEventListener('pointermove', dragMove); removeEventListener('pointerup', drop); removeEventListener('pointercancel', cancel);
      if (commit) dragEnd(); else drag = null;
    };
    const drop = finish(true), cancel = finish(false);
    addEventListener('pointermove', dragMove); addEventListener('pointerup', drop); addEventListener('pointercancel', cancel);
  }
  function dragMove(e: PointerEvent) {
    if (!drag || !root) return;
    let best = drag.over, distance = Infinity;
    for (const item of root.querySelectorAll<HTMLElement>(`[data-drag="${drag.kind}"]`)) {
      const r = item.getBoundingClientRect();
      const d = Math.hypot(e.clientX - (r.left + r.width / 2), e.clientY - (r.top + r.height / 2));
      if (d < distance) { distance = d; best = item.dataset.key!; }
    }
    drag.over = best;
  }
  function dragEnd() {
    if (!drag) return;
    const { kind, key, over } = drag;
    drag = null;
    const ids = order(kind);
    move(kind, key, ids.indexOf(over) - ids.indexOf(key));
  }

  function duration(ms: number) {
    const s = Math.round(ms / 1000), h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60, pad = (n: number) => String(n).padStart(2, '0');
    return h ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`;
  }
  function status(v: Video) {
    if (v.deleting) return '삭제 미완료 · 다시 시도해 주세요';
    if (v.active) return '처리 중 · 삭제할 수 없음';
    const versions = v.versions && v.versions.length > 1 ? ` · 저장본 ${v.versions.length}개` : '';
    return v.saved ? `저장 자막 있음${versions}` : '자막 없음 · 열어도 번역하지 않음';
  }

  function focusConfirmation(node: HTMLElement) { node.focus(); }
  function reveal(node: HTMLElement) { node.scrollIntoView?.({ block: 'nearest' }); }

  onMount(() => {
    document.title = '보관함 · 영상 번역 오버레이';
    const refresh = () => { if (busy) refreshPending = true; else load(); };
    const accountChanged = () => {
      generation++; busy = false; confirmation = null; importing = false;
      videos = []; playlists = []; selected = null; owner = undefined;
      legacyAvailable = false; accountEmail = null; error = ''; screen = 'loading'; load();
    };
    channel = new BroadcastChannel('vto-library');
    channel.onmessage = refresh;
    window.addEventListener('vto-video-deleted', refresh);
    window.addEventListener('vto-library-changed', refresh);
    window.addEventListener('vto-account-changed', accountChanged);
    load();
    return () => { channel.close(); window.removeEventListener('vto-video-deleted', refresh); window.removeEventListener('vto-library-changed', refresh); window.removeEventListener('vto-account-changed', accountChanged); };
  });
  onDestroy(() => { retired = true; generation++; });
</script>

{#snippet handle(kind: 'playlist'|'entry', key: string)}
  <!-- Pointer only; 위로 and 아래로 beside it are the keyboard path. -->
  <span aria-hidden="true" title="끌어서 옮기기" class="flex size-11 shrink-0 cursor-grab touch-none items-center justify-center rounded-lg text-ink/70 hover:bg-ink/10"
    onpointerdown={e => dragStart(e, kind, key)}>
    <svg viewBox="0 0 16 16" class="size-5" fill="currentColor"><circle cx="5" cy="3" r="1.5"/><circle cx="11" cy="3" r="1.5"/><circle cx="5" cy="8" r="1.5"/><circle cx="11" cy="8" r="1.5"/><circle cx="5" cy="13" r="1.5"/><circle cx="11" cy="13" r="1.5"/></svg>
  </span>
{/snippet}

{#snippet steps(kind: 'playlist'|'entry', key: string, label: string)}
  {@const ids = order(kind)}
  {@const at = ids.indexOf(key)}
  <button class="{button} px-2" disabled={busy || at === 0} aria-label={`${label} 위로`} onclick={() => move(kind, key, -1)}>위로</button>
  <button class="{button} px-2" disabled={busy || at === ids.length - 1} aria-label={`${label} 아래로`} onclick={() => move(kind, key, 1)}>아래로</button>
{/snippet}

{#snippet cover(key: string | undefined)}
  {@const v = key === undefined ? undefined : byKey.get(key)}
  <span class="block w-16 shrink-0 md:w-20">{#key key}<Thumb videoKey={v?.video_key} pageUrl={v?.page_url} small />{/key}</span>
{/snippet}

{#snippet rowLabel(title: string, count: number)}
  <span class="min-w-0 flex-1"><span class="line-clamp-2 break-words text-base">{title}</span> <span class="block text-sm font-normal text-ink/70">영상 {count}개</span></span>
{/snippet}

{#snippet toolbar()}
  <div class="flex flex-wrap items-center gap-2">
    {#if wide.current}
      <h2 class="mr-auto min-w-0 basis-full break-words text-lg font-semibold lg:basis-auto">{playlist?.name ?? '전체 영상'} <span class="text-sm font-normal text-ink/70">영상 {listed.length}개</span></h2>
    {/if}
    <label class="min-w-0 flex-1 md:max-w-72"><span class="sr-only">제목 또는 주소 검색</span>
      <input class="{input} w-full" type="search" placeholder="제목 또는 주소 검색" bind:value={search} /></label>
    <button class={button} aria-expanded={adding} disabled={busy} onclick={() => { adding = !adding; editing = false; }}>추가</button>
    <button class="{button} {editing ? 'bg-primary font-semibold text-surface' : ''}" disabled={busy} onclick={() => { editing = !editing; adding = false; }}>{editing ? '완료' : '편집'}</button>
  </div>
{/snippet}

{#snippet addPanel()}
  <section use:reveal aria-label="추가" class="space-y-4 rounded-lg border border-ink/20 p-4">
    <div class="flex items-center gap-2">
      <h2 class="min-w-0 flex-1 break-words text-lg font-semibold">{playlist ? `“${playlist.name}”에 추가` : '추가'}</h2>
      <button class={button} onclick={() => (adding = false)}>닫기</button>
    </div>
    {#if playlist}
      {@const target = playlist}
      {@const candidates = videos.filter(v => v.saved && !target.entries.includes(v.video_key))}
      <div class="space-y-2">
        <h3 class="text-base font-semibold">저장된 영상</h3>
        {#if candidates.length}
          <ul class="grid gap-2 sm:grid-cols-2">
            {#each candidates as v (v.video_key)}
              <li><button class="flex w-full items-center gap-2 rounded-lg border border-ink/20 p-2 text-left hover:bg-ink/5 disabled:opacity-40" disabled={busy}
                onclick={() => change({action:'add_entry', playlist_id:target.id, page_url:v.page_url, title:v.title})}>
                <span class="block w-24 shrink-0"><Thumb videoKey={v.video_key} pageUrl={v.page_url} small /></span>
                <span class="line-clamp-2 min-w-0 flex-1 break-words text-base">{v.title}</span>
                <span class="shrink-0 text-sm text-primary">추가</span>
              </button></li>
            {/each}
          </ul>
        {:else}<p class="text-sm text-ink/70">넣을 수 있는 저장 영상이 없습니다.</p>{/if}
      </div>
      <form class="flex flex-col gap-2" onsubmit={e => { e.preventDefault(); change({action:'add_entry', playlist_id:target.id, page_url:url, title:linkTitle}); }}>
        <label class="flex flex-col gap-2">영상 링크<input class={input} type="url" bind:value={url} required disabled={busy} placeholder="https://…" /></label>
        <label class="flex flex-col gap-2">영상 이름 (선택)<input class={input} bind:value={linkTitle} maxlength="200" disabled={busy} /></label>
        <p class="text-sm text-ink/80">유튜브 영상 또는 지원하는 영상 링크를 추가하세요. 링크 추가와 열기는 번역을 시작하지 않습니다. 번역은 영상감지를 눌러 시작하며 API 비용이 발생할 수 있습니다.</p>
        <button class="{button} self-start" disabled={busy || !url.trim()}>링크 추가</button>
      </form>
    {:else}
      <p class="text-sm text-ink/70">재생목록을 고르면 저장 영상이나 링크를 넣을 수 있습니다.</p>
    {/if}
    <form class="flex flex-wrap items-end gap-2 border-t border-ink/15 pt-4" onsubmit={e => { e.preventDefault(); change({action:'create_playlist', name}); }}>
      <label class="flex min-w-0 flex-1 basis-48 flex-col gap-2">새 재생목록 이름<input class={input} bind:value={name} maxlength="200" required disabled={busy} /></label>
      <button class={button} disabled={busy || !name.trim()}>+ 새 재생목록</button>
    </form>
  </section>
{/snippet}

{#snippet playlistEdit(p: Playlist)}
  <form class="flex flex-wrap items-end gap-2 rounded-lg border border-ink/20 p-4" onsubmit={e => { e.preventDefault(); change({action:'rename_playlist', playlist_id:p.id, name:rename}); }}>
    <label class="flex min-w-0 flex-1 basis-48 flex-col gap-2">재생목록 이름<input class={input} bind:value={rename} maxlength="200" required disabled={busy} /></label>
    <button class={button} disabled={busy || !rename.trim()}>이름 바꾸기</button>
    <button type="button" class="{button} border-accent/60 text-accent" disabled={busy} onclick={() => confirmation = {kind:'playlist', id:p.id, title:p.name}}>재생목록 삭제</button>
  </form>
{/snippet}

{#snippet notices()}
  {#if error}<p role="alert" class="rounded-lg border border-accent p-4 text-accent">{error}</p>{/if}
  {#if confirmation}
    <div tabindex="-1" use:focusConfirmation role="alertdialog" aria-modal="false" aria-labelledby="confirm-title" aria-describedby="confirm-description" class="space-y-4 rounded-lg border border-accent p-4">
      <h2 id="confirm-title" class="text-lg font-semibold">“{confirmation.title}” 삭제</h2>
      <p id="confirm-description">{confirmation.kind === 'video' ? '원문, 번역 자막, 용어표, 작업 기록, 영상별 파일과 모든 재생목록의 항목을 영구 삭제합니다. 되돌릴 수 없으며 나중에 다시 번역하면 API 비용이 발생합니다.' : '이 재생목록과 항목을 지웁니다. 영상의 저장된 번역은 유지됩니다.'}</p>
      <div class="flex flex-wrap gap-2">
        <button class={button} disabled={busy} onclick={() => confirmation=null}>취소</button>
        <button class="{button} border-accent text-accent" disabled={busy} onclick={() => change(confirmation!.kind === 'video' ? {action:'delete_video',video_key:confirmation!.id,title:confirmation!.title,confirmed:true} : {action:'delete_playlist',playlist_id:confirmation!.id})}>영구 삭제 확인</button>
      </div>
    </div>
  {/if}
{/snippet}

{#snippet legacy()}
  {#if legacyAvailable}
    <section class="space-y-3 rounded-lg border border-ink/30 p-4" aria-label="기존 보관함 가져오기">
      <h2 class="text-lg font-semibold">이 기기의 기존 보관함</h2>
      <p>이전에 저장한 영상과 재생목록을 현재 Google 계정의 보관함에 복사할 수 있습니다. 원본, 개인키와 쿠키는 그대로 유지합니다. 진행 중인 작업은 가져오지 않습니다.</p>
      {#if importing}
        <p role="status"><bdi>{accountEmail ?? '현재 Google 계정'}</bdi>으로 복사할까요? 같은 계정의 연결된 기기에도 전달됩니다.</p>
        <div class="flex flex-wrap gap-2">
          <button class={button} disabled={busy} onclick={() => change({action:'import_legacy', confirmed:true})}>현재 계정에 복사</button>
          <button class={button} disabled={busy} onclick={() => importing = false}>취소</button>
        </div>
      {:else}
        <button class={button} disabled={busy} onclick={() => importing = true}>기존 보관함 가져오기</button>
      {/if}
    </section>
  {/if}
{/snippet}

{#snippet emptyList()}
  <p class="rounded-lg border border-dashed border-ink/30 p-4">{search ? '일치하는 영상이 없습니다.' : playlist ? '이 재생목록은 비어 있습니다. 추가를 눌러 저장 영상이나 링크를 넣으세요.' : '아직 저장한 영상이 없습니다. 영상을 열고 영상감지를 누르거나, 추가에서 재생목록을 만들어 링크를 넣으세요.'}</p>
{/snippet}

{#snippet videoList()}
  {#if visible.length}
    <!-- Edit mode is one column: a card is too narrow for its address and controls on one line each. -->
    <ol class="grid gap-2 {editing ? '' : 'md:grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] md:gap-4'}">
      {#each visible as video (video.video_key)}
        {@const reorderable = editing && !!playlist}
        <li data-video-key={video.video_key} data-drag={reorderable ? 'entry' : undefined} data-key={video.video_key}
          class="flex min-w-0 flex-col gap-2 rounded-lg {drag?.kind === 'entry' && drag.over === video.video_key ? 'ring-2 ring-primary' : ''} {drag?.key === video.video_key ? 'opacity-60' : ''} {editing ? 'border border-ink/20 p-2' : ''}">
          <button class="flex w-full min-w-0 gap-3 rounded-lg text-left hover:bg-ink/5 disabled:opacity-60 {editing ? '' : 'md:flex-col md:gap-2'}" disabled={busy || video.deleting}
            aria-label={`${video.title} 영상 열기`} aria-describedby={`status-${video.video_key}`} onclick={() => openVideo(video)}>
            <span class="relative block w-36 shrink-0 {editing ? 'md:w-48' : 'md:w-full'}">
              <Thumb videoKey={video.video_key} pageUrl={video.page_url} />
              {#if (video.duration_ms ?? 0) > 0}<span class="absolute right-1 bottom-1 rounded bg-surface/90 px-1 text-sm">{duration(video.duration_ms!)}</span>{/if}
            </span>
            <span class="min-w-0 flex-1 md:px-1 md:pb-1">
              <span data-title class="line-clamp-2 break-words text-base font-semibold">{video.title}</span>
              <span class="block truncate text-sm text-ink/70">{host(video.page_url)}</span>
              <span id={`status-${video.video_key}`} class="block text-sm {video.deleting || video.active ? 'text-accent' : 'text-ink/70'}">{status(video)}</span>
            </span>
          </button>
          {#if editing}
            <p class="break-all text-sm text-ink/70">{video.page_url}</p>
            {#if video.versions && video.versions.length > 1}
              <label class="flex flex-col gap-2 text-sm">
                저장된 자막 {video.versions.length}개 · 선택한 뒤 영상을 다시 열어 주세요
                <select class={input} aria-label={`${video.title} 저장 자막 선택`} disabled={busy || video.active || video.deleting}
                  value={video.versions.find(v => v.selected)?.id ?? ''}
                  onchange={(event) => change({action:'select_translation',video_key:video.video_key,translation_id:event.currentTarget.value})}>
                  {#each video.versions as version, index}
                    <option value={version.id}>저장본 {index + 1}{index === 0 ? ' · 최근 선택 또는 완료' : ''}{version.selected ? ' · 사용 중' : ''}</option>
                  {/each}
                </select>
              </label>
            {/if}
            <div class="flex flex-wrap items-center gap-2">
              {#if reorderable}
                {@render handle('entry', video.video_key)}
                {@render steps('entry', video.video_key, `${video.title} 영상`)}
                <button class={button} disabled={busy} onclick={() => change({action:'remove_entry', playlist_id:playlist!.id, video_key:video.video_key})}>목록에서 빼기</button>
              {/if}
              <button class="{button} border-accent/60 text-accent" disabled={busy || video.active} onclick={() => confirmation = {kind:'video', id:video.video_key, title:video.title}}>{video.deleting ? '삭제 다시 시도' : '영상 삭제'}</button>
            </div>
          {:else if video.deleting}
            <button class="{button} self-start border-accent/60 text-accent" disabled={busy} onclick={() => confirmation = {kind:'video', id:video.video_key, title:video.title}}>삭제 다시 시도</button>
          {/if}
        </li>
      {/each}
    </ol>
  {:else}
    {@render emptyList()}
  {/if}
{/snippet}

<Nav />
<main bind:this={root} class="mx-auto max-w-6xl p-4">
  {#if screen === 'loading'}
    <h1 class="text-2xl font-semibold">보관함</h1>
    <p role="status" aria-busy="true" class="mt-4">보관함 불러오는 중…</p>
  {:else if screen === 'error'}
    <h1 class="text-2xl font-semibold">보관함</h1>
    <div class="mt-4 space-y-4">
      {@render notices()}
      <button class={button} onclick={() => { error = ''; load(); }}>다시 불러오기</button>
    </div>
  {:else if wide.current}
    <div class="grid grid-cols-[16rem_minmax(0,1fr)] items-start gap-6">
      <aside class="sticky top-0 max-h-dvh space-y-4 overflow-y-auto pb-4">
        <!-- As tall as the pane's toolbar, so the first playlist and the first card share a top edge. -->
        <div class="flex min-h-11 flex-col justify-center">
          <h1 class="text-2xl font-semibold">보관함</h1>
          {#if accountEmail}<p class="mt-1 truncate text-sm text-ink/70">Google 계정: <bdi>{accountEmail}</bdi></p>{/if}
        </div>
        <ul aria-label="재생목록" class="space-y-1">
          {#each [{id:'', name:'전체 영상', entries:videos.map(v => v.video_key)}, ...playlists] as p (p.id)}
            {@const current = open === p.id}
            <li data-drag={p.id ? 'playlist' : undefined} data-key={p.id}
              class="rounded-lg {drag?.kind === 'playlist' && drag.over === p.id ? 'ring-2 ring-primary' : ''} {drag?.key === p.id && p.id ? 'opacity-60' : ''}">
              <div class="flex items-center gap-1">
                {#if editing}{#if p.id}{@render handle('playlist', p.id)}{:else}<span class="w-11 shrink-0"></span>{/if}{/if}
                <button aria-current={current ? 'true' : undefined} disabled={busy} onclick={() => choose(p.id)}
                  class="flex min-h-11 w-full min-w-0 items-center gap-3 rounded-lg border-l-4 p-2 text-left {current ? 'border-primary bg-primary/15 font-semibold' : 'border-transparent hover:bg-ink/5'}">
                  {@render cover(p.entries[0])}
                  {@render rowLabel(p.name, p.entries.length)}
                </button>
              </div>
              {#if editing && p.id}<div class="flex gap-2 py-1 pl-12">{@render steps('playlist', p.id, `${p.name} 재생목록`)}</div>{/if}
            </li>
          {/each}
        </ul>
        {@render legacy()}
      </aside>
      <section aria-label="영상 목록" class="min-w-0 space-y-4">
        {@render toolbar()}
        {@render notices()}
        {#if adding}{@render addPanel()}{/if}
        {#if editing && playlist}{@render playlistEdit(playlist)}{/if}
        {@render videoList()}
      </section>
    </div>
  {:else}
    <div class="space-y-4">
      <div>
        <h1 class="text-2xl font-semibold">보관함</h1>
        {#if accountEmail}<p class="mt-1 break-words text-sm text-ink/70">Google 계정: <bdi>{accountEmail}</bdi></p>{/if}
      </div>
      {@render toolbar()}
      {@render legacy()}
      {@render notices()}
      {#if adding && !playlist}{@render addPanel()}{/if}
      {#if !videos.length && !playlists.length && !search}{@render emptyList()}{/if}
      <ul aria-label="재생목록" class="space-y-2">
        {#each [{id:'', name:'전체 영상', entries:videos.map(v => v.video_key)}, ...playlists] as p (p.id)}
          {@const expanded = open === p.id}
          <li data-drag={p.id ? 'playlist' : undefined} data-key={p.id} class="rounded-lg {drag?.kind === 'playlist' && drag.over === p.id ? 'ring-2 ring-primary' : ''}">
            <div class="flex flex-wrap items-center gap-1 rounded-lg bg-surface {expanded ? 'sticky top-0 z-10 shadow-[0_8px_8px_-8px_rgb(0_0_0/0.6)]' : ''}">
              {#if editing}{#if p.id}{@render handle('playlist', p.id)}{:else}<span class="w-11 shrink-0"></span>{/if}{/if}
              <button aria-expanded={expanded} disabled={busy} onclick={() => choose(p.id)}
                class="flex min-h-11 min-w-0 flex-1 items-center gap-3 rounded-lg border-l-4 p-2 text-left {expanded ? 'border-primary bg-primary/15 font-semibold' : 'border-transparent bg-ink/5'}">
                {@render cover(p.entries[0])}
                {@render rowLabel(p.name, p.entries.length)}
                <svg aria-hidden="true" viewBox="0 0 16 16" class="size-4 shrink-0 {expanded ? 'rotate-180' : ''}" fill="none" stroke="currentColor" stroke-width="2"><path d="m4 6 4 4 4-4" stroke-linecap="round" stroke-linejoin="round" /></svg>
              </button>
              {#if editing && p.id}<div class="flex basis-full justify-end gap-2 pb-1">{@render steps('playlist', p.id, `${p.name} 재생목록`)}</div>{/if}
            </div>
            {#if expanded}
              <div class="space-y-2 pt-2">
                {#if adding && playlist}{@render addPanel()}{/if}
                {#if editing && playlist}{@render playlistEdit(playlist)}{/if}
                {@render videoList()}
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</main>
