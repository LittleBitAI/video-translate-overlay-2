<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { request } from './api';
  import { go } from './router.svelte';
  import Link from './Link.svelte';

  type Video = { video_key: string; title: string; page_url: string; saved: boolean; active: boolean; deleting: boolean };
  type Playlist = { id: string; name: string; entries: string[] };
  let videos = $state<Video[]>([]);
  let playlists = $state<Playlist[]>([]);
  let selected = $state('');
  let search = $state('');
  let name = $state('');
  let rename = $state('');
  let url = $state('');
  let linkTitle = $state('');
  let savedKey = $state('');
  let screen = $state<'loading'|'ready'|'error'>('loading');
  let error = $state('');
  let busy = $state(false);
  let confirmation = $state<{ kind: 'video'|'playlist'; id: string; title: string } | null>(null);
  let generation = 0;
  let channel: BroadcastChannel;
  let retired = false;
  const playlist = $derived(playlists.find(p => p.id === selected));
  const visible = $derived((playlist ? playlist.entries.map(k => videos.find(v => v.video_key === k)).filter((v): v is Video => !!v) : videos)
    .filter(v => `${v.title} ${v.page_url}`.toLocaleLowerCase().includes(search.toLocaleLowerCase())));
  const button = 'min-h-11 rounded-lg border border-ink/30 px-4 py-2 text-base disabled:opacity-40';
  const input = 'min-h-11 min-w-0 rounded-lg border border-ink/30 bg-ink/5 px-4 py-2 text-base';

  async function load() {
    const mine = ++generation;
    screen = 'loading';
    const r = await request('GET', '/library/data');
    if (mine !== generation) return;
    if (r.status !== 200) { screen = 'error'; error = r.body?.message ?? '보관함을 불러오지 못했습니다.'; return; }
    videos = r.body.videos;
    playlists = r.body.playlists;
    if (!playlists.some(p => p.id === selected)) selected = '';
    screen = 'ready';
  }

  async function change(action: Record<string, unknown>) {
    if (busy) return;
    busy = true;
    error = '';
    const mine = ++generation;
    const r = await request('POST', '/library/data', action);
    if (action.action === 'delete_video' && (r.status === 200 || r.status >= 500)) {
      if (!retired) channel.postMessage(action.video_key);
    }
    if (mine !== generation) return;
    busy = false;
    if (r.status !== 200) {
      error = r.body?.message ?? '변경하지 못했습니다. 다시 시도해 주세요.';
      if (action.action === 'delete_video' && r.status >= 500) await load();
      return;
    }
    confirmation = null;
    name = ''; url = ''; linkTitle = ''; savedKey = '';
    await load();
  }

  async function openVideo(video: Video) {
    busy = true; error = '';
    const mine = ++generation;
    const r = await request('POST', '/browser/open', { url: video.page_url });
    if (mine !== generation) return;
    busy = false;
    if (r.status === 200) go('/');
    else error = r.body?.message ?? '영상을 열지 못했습니다.';
  }

  function choose(id: string) {
    selected = id;
    rename = playlists.find(p => p.id === id)?.name ?? '';
    search = ''; url = ''; linkTitle = ''; savedKey = ''; confirmation = null; error = '';
  }

  function focusConfirmation(node: HTMLElement) { node.focus(); }

  onMount(() => {
    const refresh = () => { if (!busy) load(); };
    channel = new BroadcastChannel('vto-library');
    channel.onmessage = refresh;
    window.addEventListener('vto-video-deleted', refresh);
    load();
    return () => { channel.close(); window.removeEventListener('vto-video-deleted', refresh); };
  });
  onDestroy(() => { retired = true; generation++; });
</script>

<header class="border-b border-ink/10">
  <nav aria-label="주 메뉴" class="mx-auto flex max-w-5xl flex-wrap gap-2 p-4">
    <Link href="/" class={button}>영상 열기</Link>
    <Link href="/preferences" class={button}>설정</Link>
  </nav>
</header>
<main class="mx-auto max-w-5xl space-y-6 p-4">
  <div>
    <h1 class="text-2xl font-semibold">보관함</h1>
    <p class="mt-2 text-base text-ink/80">저장한 자막을 다시 보고, 주제별로 영상을 골라 보세요. 이 기기에만 저장됩니다.</p>
  </div>
  {#if error}<p role="alert" class="rounded-lg border border-accent p-4 text-accent">{error}</p>{/if}
  {#if screen === 'loading'}
    <p role="status" aria-busy="true">보관함 불러오는 중…</p>
  {:else if screen === 'error'}
    <button class={button} onclick={() => { error = ''; load(); }}>다시 불러오기</button>
  {:else}
    <section aria-label="재생목록" class="space-y-4">
      <h2 class="text-lg font-semibold">재생목록</h2>
      <button class={button} aria-pressed={!selected} disabled={busy} onclick={() => choose('')}>전체 영상</button>
      <ol class="space-y-2">
        {#each playlists as p, i (p.id)}
          <li class="flex flex-wrap items-center gap-2">
            <button class="{button} max-w-full break-words" disabled={busy} aria-pressed={selected === p.id} onclick={() => choose(p.id)}>{p.name} ({p.entries.length})</button>
            <button class={button} disabled={busy || i === 0} aria-label={`${p.name} 재생목록 위로`} onclick={() => change({action:'move_playlist',playlist_id:p.id,delta:-1})}>↑</button>
            <button class={button} disabled={busy || i === playlists.length-1} aria-label={`${p.name} 재생목록 아래로`} onclick={() => change({action:'move_playlist',playlist_id:p.id,delta:1})}>↓</button>
          </li>
        {/each}
      </ol>
      <form class="flex flex-wrap gap-2" onsubmit={e => { e.preventDefault(); change({action:'create_playlist',name}); }}>
        <label class="flex min-w-0 flex-1 basis-full flex-col gap-2 sm:basis-0">새 재생목록 이름<input class={input} bind:value={name} maxlength="200" required disabled={busy} /></label>
        <button class="{button} self-end" disabled={busy || !name.trim()}>재생목록 만들기</button>
      </form>
    </section>
    {#if playlist}
      <section class="space-y-4 rounded-lg border border-ink/20 p-4" aria-label="선택한 재생목록">
        <h2 class="text-lg font-semibold">{playlist.name}</h2>
        <form class="flex flex-wrap gap-2" onsubmit={e => { e.preventDefault(); change({action:'rename_playlist',playlist_id:selected,name:rename}); }}>
          <label class="flex min-w-0 flex-1 basis-full flex-col gap-2 sm:basis-0">재생목록 이름<input class={input} bind:value={rename} maxlength="200" required disabled={busy} /></label>
          <button class="{button} self-end" disabled={busy || !rename.trim()}>이름 바꾸기</button>
          <button type="button" class="{button} self-end" disabled={busy} onclick={() => confirmation={kind:'playlist',id:selected,title:playlist!.name}}>재생목록 삭제</button>
        </form>
        <form class="flex flex-wrap gap-2" onsubmit={e => { e.preventDefault(); const v=videos.find(v => v.video_key===savedKey); if(v) change({action:'add_entry',playlist_id:selected,page_url:v.page_url,title:v.title}); }}>
          <label class="flex min-w-0 flex-1 basis-full flex-col gap-2 sm:basis-0">저장된 영상<select aria-label="저장된 영상" class={input} bind:value={savedKey} disabled={busy}>
            <option value="">영상을 고르세요</option>
            {#each videos.filter(v => v.saved && !playlist!.entries.includes(v.video_key)) as v}<option value={v.video_key}>{v.title}</option>{/each}
          </select></label>
          <button class="{button} self-end" disabled={busy || !savedKey}>저장 영상 추가</button>
        </form>
        <form class="flex flex-col gap-2" onsubmit={e => { e.preventDefault(); change({action:'add_entry',playlist_id:selected,page_url:url,title:linkTitle}); }}>
          <label class="flex flex-col gap-2">영상 링크<input class={input} type="url" bind:value={url} required disabled={busy} placeholder="https://…" /></label>
          <label class="flex flex-col gap-2">영상 이름 (선택)<input class={input} bind:value={linkTitle} maxlength="200" disabled={busy} /></label>
          <p class="text-sm text-ink/80">유튜브 영상 또는 지원하는 영상 링크를 추가하세요. 링크 추가와 열기는 번역을 시작하지 않습니다. 번역은 영상감지를 눌러 시작하며 API 비용이 발생할 수 있습니다.</p>
          <button class="{button} self-start" disabled={busy || !url.trim()}>링크 추가</button>
        </form>
      </section>
    {/if}
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
    <section aria-label="영상 목록" class="space-y-4">
      <h2 class="text-lg font-semibold">{playlist ? '재생목록 영상' : '전체 영상'}</h2>
      <label class="flex flex-col gap-2">제목 또는 주소 검색<input class={input} type="search" bind:value={search} /></label>
      {#if !visible.length}<p class="rounded-lg border border-dashed border-ink/30 p-4">{search ? '일치하는 영상이 없습니다.' : playlist ? '이 재생목록에 저장 영상이나 링크를 추가하세요.' : '아직 저장한 영상이 없습니다. 영상을 열고 영상감지를 누르거나 재생목록에 링크를 추가하세요.'}</p>{/if}
      <ol class="space-y-4">
        {#each visible as video (video.video_key)}
          <li class="space-y-2 rounded-lg border border-ink/20 p-4" data-video-key={video.video_key}>
            <h3 class="text-base font-semibold">{video.title}</h3>
            <p class="break-all text-sm text-ink/80">{video.page_url}</p>
            <p class="text-sm">{video.deleting ? '삭제 미완료 · 다시 시도해 주세요' : video.active ? '처리 중 · 삭제할 수 없음' : video.saved ? '저장 자막 있음 · 재번역 없이 열기' : '저장 자막 없음 · 열어도 자동 번역하지 않음'}</p>
            <div class="flex flex-wrap gap-2">
              <button class="{button} bg-primary font-semibold text-surface" disabled={busy || video.deleting} onclick={() => openVideo(video)}>영상 열기</button>
              {#if playlist}
                <button class={button} disabled={busy || playlist.entries.indexOf(video.video_key)===0} aria-label={`${video.title} 영상 위로`} onclick={() => change({action:'move_entry',playlist_id:selected,video_key:video.video_key,delta:-1})}>↑</button>
                <button class={button} disabled={busy || playlist.entries.indexOf(video.video_key)===playlist.entries.length-1} aria-label={`${video.title} 영상 아래로`} onclick={() => change({action:'move_entry',playlist_id:selected,video_key:video.video_key,delta:1})}>↓</button>
                <button class={button} disabled={busy} onclick={() => change({action:'remove_entry',playlist_id:selected,video_key:video.video_key})}>목록에서 빼기</button>
              {/if}
              <button class={button} disabled={busy || video.active} onclick={() => confirmation={kind:'video',id:video.video_key,title:video.title}}>{video.deleting ? '삭제 다시 시도' : '영상 삭제'}</button>
            </div>
          </li>
        {/each}
      </ol>
    </section>
  {/if}
</main>
