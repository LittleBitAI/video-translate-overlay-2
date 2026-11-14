// Overlay contract tests. They run in Chromium (the engine of Android WebView
// and WebView2) and load the overlay at run time from `src/overlay/index.ts`,
// which must export:
//
//   mountOverlay({
//     video: HTMLVideoElement,     // the page's main video
//     pageUrl: string,
//     title: string,
//     mediaUrl: string | null,
//     request: (method: string, path: string, body?: unknown)
//       => Promise<{ status: number; body: any }>,   // P6, P7, P8 of docs/design/api.yaml
//   }): { destroy(): void } | Promise<{ destroy(): void }>
//
// The overlay's 영상감지 button carries `data-vto="detect"` and the subtitle box
// `data-vto="subtitle"` (light DOM or an open shadow root). A subtitle counts as
// shown when that box is visible and its text is not empty.

import { expect, onTestFinished, test, vi } from 'vitest';
import { page } from 'vitest/browser';

type Reply = { status: number; body: any };
type Call = { method: string; path: string; body?: any };
type Mount = (opts: {
  video: HTMLVideoElement;
  pageUrl: string;
  title: string;
  mediaUrl: string | null;
  request: (method: string, path: string, body?: unknown) => Promise<Reply>;
}) => { destroy(): void } | Promise<{ destroy(): void }>;

const OVERLAY_MODULE = '/src/overlay/index.ts';
const YT = 'https://www.youtube.com/watch?v=overlay0001';
const TITLE = 'Attention Is All You Need, explained';
const VIDEO_KEY = 'vk-overlay-0001';
const MAIN_MS = 120_000;
const CUES = [
  { seq: 1, start_ms: 1_000, end_ms: 3_000, text_ko: '첫 번째 문장입니다.' },
  { seq: 2, start_ms: 5_000, end_ms: 8_000, text_ko: '두 번째 문장입니다.' },
  { seq: 3, start_ms: 60_000, end_ms: 63_000, text_ko: '뒤쪽에서 나오는 문장입니다.' },
  { seq: 4, start_ms: 70_000, end_ms: 72_000, text_ko: '더 뒤쪽의 문장입니다.' },
];
const LONG_CUE = {
  seq: 1,
  start_ms: 2_000,
  end_ms: 6_000,
  text_ko:
    '이 강의에서는 트랜스포머의 어텐션 메커니즘이 긴 문맥에서 어떻게 계산량을 줄이는지, 그리고 그 과정에서 생기는 근사 오차를 어떻게 다루는지 차례대로 설명합니다.',
};

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function mountOverlay(opts: Parameters<Mount>[0]) {
  const mod = await import(/* @vite-ignore */ OVERLAY_MODULE);
  if (typeof mod.mountOverlay !== 'function') throw new Error(`${OVERLAY_MODULE} does not export mountOverlay`);
  const handle = await (mod.mountOverlay as Mount)(opts);
  let destroyed = false;
  const destroy = () => {
    if (destroyed) return;
    destroyed = true;
    handle?.destroy?.();
  };
  onTestFinished(destroy);
  return { destroy };
}

/** A page with a player whose clock the test drives. */
function makePlayer(durationS: number) {
  document.body.innerHTML = '';
  document.body.style.margin = '0';
  const player = document.createElement('div');
  player.id = 'movie_player';
  player.className = 'html5-video-player';
  player.style.cssText = 'position:relative;width:100%;max-width:1280px;aspect-ratio:16/9;background:#000';
  const video = document.createElement('video');
  video.style.cssText = 'display:block;width:100%;height:100%';
  player.append(video);
  const below = document.createElement('p');
  below.textContent = 'Lecture description and comments.';
  document.body.append(player, below);

  const state = { time: 0, duration: durationS, paused: true, seeking: false, rate: 1 };
  const writes: string[] = [];
  const define = (name: string, get: () => unknown, set?: (v: any) => void) =>
    Object.defineProperty(video, name, { configurable: true, get, set });
  define('currentTime', () => state.time, (v) => { writes.push(`currentTime=${v}`); state.time = Number(v); });
  define('playbackRate', () => state.rate, (v) => { writes.push(`playbackRate=${v}`); state.rate = Number(v); });
  define('duration', () => state.duration);
  define('paused', () => state.paused);
  define('seeking', () => state.seeking);
  define('readyState', () => 4);
  video.play = async () => { writes.push('play()'); };
  video.pause = () => { writes.push('pause()'); };
  const fire = (...names: string[]) => names.forEach((n) => video.dispatchEvent(new Event(n)));

  return {
    player,
    video,
    writes,
    play(at: number) { state.time = at; state.paused = false; fire('play', 'playing', 'timeupdate'); },
    tick(at: number) { state.time = at; fire('timeupdate'); },
    seek(at: number) {
      state.seeking = true; fire('seeking');
      state.time = at; state.seeking = false; fire('seeked', 'timeupdate');
    },
    pause() { state.paused = true; fire('pause', 'timeupdate'); },
    /** The element switches to another media (an ad, or back to the lecture). */
    load(seconds: number) {
      state.duration = seconds; state.time = 0;
      fire('emptied', 'loadstart', 'durationchange', 'loadedmetadata', 'loadeddata', 'canplay');
    },
  };
}

function fakeBackend(handle: (call: Call) => Reply | Promise<Reply>) {
  const calls: Call[] = [];
  const request = async (method: string, path: string, body?: unknown): Promise<Reply> => {
    const call = { method: method.toUpperCase(), path, body };
    calls.push(call);
    return structuredClone(await handle(call));
  };
  return { calls, request };
}

const route = (c: Call) => `${c.method} ${c.path.split('?')[0]}`;
const job = (stage: string, percent: number) => ({
  job_id: 'job-0001', video_key: VIDEO_KEY, title: TITLE, stage, percent, failure_reason: null,
  input_amount: 0, output_amount: 0, cost_krw: 0, notified: stage === 'ready',
});
const subtitlesBody = (cues: unknown[]) => ({ video_key: VIDEO_KEY, duration_ms: MAIN_MS, source_language: 'en', cues });

/** A video whose subtitles are already made. */
function readyBackend() {
  return fakeBackend((c) => {
    switch (route(c)) {
      case 'POST /jobs': return { status: 200, body: { job_id: 'job-0001', video_key: VIDEO_KEY, stage: 'ready', reused: true } };
      case 'GET /jobs': return { status: 200, body: job('ready', 100) };
      case `GET /videos/${VIDEO_KEY}/subtitles`: return { status: 200, body: subtitlesBody(CUES) };
      default: return { status: 404, body: { error: 'not found' } };
    }
  });
}

test('ready subtitles offer translation repair without fetching or rewriting the audio', async () => {
  const player = makePlayer(MAIN_MS / 1000);
  const backend = readyBackend();
  await mountOverlay({ video: player.video, pageUrl: YT, title: TITLE, mediaUrl: null, request: backend.request });
  const detect = () => deepAll('[data-vto="detect"]')[0] as HTMLButtonElement;
  await expect.poll(() => detect()?.textContent).toBe('다시 번역');
  expect(detect().title).toContain('API 비용');
  detect().click();
  await expect.poll(() => backend.calls.filter(c => c.method === 'POST').length).toBe(1);
  expect(backend.calls.find(c => c.method === 'POST')?.body).toEqual({ page_url: YT, retranslate: true });
  await expect.poll(() => detect()?.disabled).toBe(false);
});

test.each(['ready', 'failed'])('deleting another video preserves cues through a failed refresh: %s', async stage => {
  const player = makePlayer(MAIN_MS / 1000);
  let failing = false;
  const backend = fakeBackend(c => c.path === '/jobs'
    ? failing ? {status: 500, body: {message: 'Unavailable'}} : {status: 200, body: job(stage, 100)}
    : {status: 200, body: subtitlesBody(CUES)});
  await mountOverlay({video: player.video, pageUrl: YT, title: TITLE, mediaUrl: null, request: backend.request});
  player.seek(1.5);
  await expect.poll(shownSubtitle).toBe(CUES[0].text_ko);
  const reads = backend.calls.filter(c => c.path === '/jobs').length;
  failing = true;
  window.dispatchEvent(new CustomEvent('vto-video-deleted', {detail: 'other-video'}));
  await expect.poll(() => backend.calls.filter(c => c.path === '/jobs').length, {timeout: 2500}).toBeGreaterThan(reads);
  expect(shownSubtitle()).toBe(CUES[0].text_ko);
  expect(backend.calls.filter(c => c.path.endsWith('/subtitles'))).toHaveLength(1);
  expect(backend.calls.filter(c => c.method === 'POST')).toHaveLength(0);
});

test.each(['ready', 'failed'])('deletion rejects a late saved-subtitle response and status: %s', async (stage) => {
  const player=makePlayer(MAIN_MS/1000);
  let deleted=false;
  let reply: (r: Reply) => void=()=>{};
  let requested=false;
  const request = async (method: string,path: string): Promise<Reply> => {
    if(path==='/jobs') return deleted ? {status:410,body:{message:'Deleted',deletion_token:'new-token'}} : {status:200,body:job(stage,100)};
    requested=true;
    return new Promise(resolve=>{reply=resolve;});
  };
  await mountOverlay({video:player.video,pageUrl:YT,title:TITLE,mediaUrl:null,request});
  player.seek(1.5);
  await expect.poll(()=>requested).toBe(true);
  deleted=true;
  window.dispatchEvent(new CustomEvent('vto-video-deleted',{detail:VIDEO_KEY}));
  await expect.poll(()=>deepAll('[data-vto="status"]')[0]?.textContent).toBe('Deleted');
  reply({status:200,body:subtitlesBody(CUES)});
  await sleep(50);
  expect(shownSubtitle()).toBe('');
  expect(deepAll('[data-vto="status"]')[0]?.textContent).toBe('Deleted');
  expect(deepAll('[data-vto="detect"]')[0]?.textContent).toBe('영상감지');
});

test('deletion before the first job response cannot revive that video', async () => {
  const player = makePlayer(MAIN_MS / 1000);
  let first = true;
  let resolveJob: (reply: Reply) => void = () => {};
  const backend = fakeBackend(c => {
    if (c.path === '/jobs') {
      if (first) { first = false; return new Promise(resolve => { resolveJob = resolve; }); }
      return {status: 410, body: {message: 'Deleted', deletion_token: 'new-token'}};
    }
    return {status: 200, body: subtitlesBody(CUES)};
  });
  await mountOverlay({video: player.video, pageUrl: YT, title: TITLE, mediaUrl: null, request: backend.request});
  player.seek(1.5);
  await expect.poll(() => first).toBe(false);
  window.dispatchEvent(new CustomEvent('vto-video-deleted', {detail: VIDEO_KEY}));
  resolveJob({status: 200, body: job('ready', 100)});
  await expect.poll(() => deepAll('[data-vto="status"]')[0]?.textContent, {timeout: 2500}).toBe('Deleted');
  expect(shownSubtitle()).toBe('');
  expect(backend.calls.filter(c => c.path.endsWith('/subtitles'))).toHaveLength(0);
});

test('explicit detection after deletion can display newly created subtitles', async () => {
  const player = makePlayer(MAIN_MS / 1000);
  let state = 'ready';
  const backend = fakeBackend(c => {
    if (c.method === 'POST') { state = 'recreated'; return {status: 200, body: job('ready', 100)}; }
    if (c.path === '/jobs') return state === 'deleted'
      ? {status: 410, body: {message: 'Deleted', deletion_token: 'new-token'}} : {status: 200, body: job('ready', 100)};
    return {status: 200, body: subtitlesBody(CUES)};
  });
  const consent = vi.spyOn(window, 'confirm').mockReturnValue(true);
  onTestFinished(() => consent.mockRestore());
  await mountOverlay({video: player.video, pageUrl: YT, title: TITLE, mediaUrl: null, request: backend.request});
  player.seek(1.5);
  await expect.poll(shownSubtitle).toBe(CUES[0].text_ko);
  state = 'deleted';
  window.dispatchEvent(new CustomEvent('vto-video-deleted', {detail: VIDEO_KEY}));
  await expect.poll(() => deepAll('[data-vto="status"]')[0]?.textContent).toBe('Deleted');
  (deepAll('[data-vto="detect"]')[0] as HTMLButtonElement).click();
  await expect.poll(shownSubtitle).toBe(CUES[0].text_ko);
  const posts = backend.calls.filter(c => c.method === 'POST');
  expect(posts).toHaveLength(1);
  expect(posts[0].body.deletion_token).toBe('new-token');
});

test('a visible saved overlay notices deletion from another screen without a native event', async () => {
  const player=makePlayer(MAIN_MS/1000);
  let deleted=false;
  const backend=fakeBackend(c => c.path==='/jobs'
    ? deleted ? {status:410,body:{message:'Deleted'}} : {status:200,body:job('ready',100)}
    : {status:200,body:subtitlesBody(CUES)});
  await mountOverlay({video:player.video,pageUrl:YT,title:TITLE,mediaUrl:null,request:backend.request});
  player.seek(1.5);
  await expect.poll(shownSubtitle).toBe(CUES[0].text_ko);
  deleted=true;
  await expect.poll(shownSubtitle,{timeout:3000}).toBe('');
});

// A failed replacement must show the completed set without another detect click,
// both after a retry on this page and when reopening an already failed job.
test.each(['retry', 'reopen'])('completed subtitles remain available after retranslation failure: %s', async (entry) => {
  for (const [reason, message] of [
    ['translate_failed', '번역하지 못했습니다.'],
    ['key_rejected', 'Gemini가 API 키를 받아 주지 않았습니다.'],
    ['cost_limit', '번역을 멈췄습니다.'],
  ]) {
    const p = makePlayer(MAIN_MS / 1000);
    let stage = entry === 'retry' ? 'ready' : 'failed';
    const backend = fakeBackend((c) => {
      switch (route(c)) {
        case 'POST /jobs':
          stage = 'translating';
          return { status: 200, body: job(stage, 0) };
        case 'GET /jobs':
          return { status: 200, body: { ...job(stage, 0), failure_reason: stage === 'failed' ? reason : null } };
        case `GET /videos/${VIDEO_KEY}/subtitles`:
          return { status: 200, body: subtitlesBody(CUES) };
        default: return { status: 404, body: {} };
      }
    });
    const handle = await mountOverlay({ video: p.video, pageUrl: YT, title: TITLE, mediaUrl: null, request: backend.request });
    p.seek(1.5);
    const detect = () => deepAll('[data-vto="detect"]')[0] as HTMLButtonElement;
    await expect.poll(shownSubtitle, SHOWN).toBe(CUES[0].text_ko);
    if (entry === 'retry') {
      detect().click();
      await expect.poll(() => deepAll('[data-vto="status"]')[0]?.textContent).toContain('한국어로 번역하는 중');
      expect(detect().disabled).toBe(true);
      expect(shownSubtitle()).toBe(CUES[0].text_ko);
      stage = 'failed';
    }
    await expect.poll(() => deepAll('[data-vto="status"]')[0]?.textContent, SHOWN).toContain(message);
    await expect.poll(shownSubtitle, SHOWN).toBe(CUES[0].text_ko);
    expect(detect().textContent).toBe('다시 번역');
    expect(detect().disabled).toBe(false);
    expect(backend.calls.filter(c => c.method === 'POST')).toHaveLength(entry === 'retry' ? 1 : 0);
    p.seek(61);
    await expect.poll(shownSubtitle, SHOWN).toBe(CUES[2].text_ko);
    detect().click();
    await expect.poll(() => backend.calls.filter(c => c.method === 'POST').length).toBe(entry === 'retry' ? 2 : 1);
    expect(backend.calls.filter(c => c.method === 'POST').at(-1)?.body).toEqual({ page_url: YT, retranslate: true });
    handle.destroy();
  }
});

test.each([409, 404, 500])('a failed job without readable saved subtitles still offers detection: %i', async (status) => {
  const p = makePlayer(MAIN_MS / 1000);
  const backend = fakeBackend((c) => route(c) === 'GET /jobs'
    ? { status: 200, body: { ...job('failed', 0), failure_reason: 'no_speech' } }
    : { status, body: { message: 'Subtitles unavailable' } });
  await mountOverlay({ video: p.video, pageUrl: YT, title: TITLE, mediaUrl: null, request: backend.request });
  await subtitlesLoaded(backend);
  await expect.poll(() => deepAll('[data-vto="status"]')[0]?.textContent).toContain('영상에서 말소리를 찾지 못했습니다.');
  expect(shownSubtitle()).toBe('');
  const detect = deepAll('[data-vto="detect"]')[0] as HTMLButtonElement;
  expect(detect.textContent).toBe('영상감지');
  expect(detect.disabled).toBe(false);
  expect(backend.calls.filter(c => c.method === 'POST')).toHaveLength(0);
});

function deepAll(selector: string, root: Document | ShadowRoot = document): Element[] {
  const found = [...root.querySelectorAll(selector)];
  for (const el of root.querySelectorAll('*')) if (el.shadowRoot) found.push(...deepAll(selector, el.shadowRoot));
  return found;
}

function visible(el: Element) {
  const r = el.getBoundingClientRect();
  return r.width > 0 && r.height > 0 && el.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true });
}

function shownSubtitle(): string {
  return deepAll('[data-vto="subtitle"]')
    .filter(visible)
    .map((el) => (el.textContent ?? '').trim())
    .filter(Boolean)
    .join('\n');
}

function expectOnScreen(el: Element, what: string) {
  const r = el.getBoundingClientRect();
  const box = `${what} at ${JSON.stringify(r)} in ${innerWidth}x${innerHeight}`;
  expect(r.width > 0 && r.height > 0, box).toBe(true);
  expect(r.left >= 0 && r.top >= 0 && r.right <= innerWidth && r.bottom <= innerHeight, box).toBe(true);
  expect(el.scrollWidth <= el.clientWidth + 1, `${what} overflows sideways`).toBe(true);
}

const SHOWN = { timeout: 3_000, interval: 50 };

async function subtitlesLoaded(backend: ReturnType<typeof fakeBackend>) {
  await expect
    .poll(() => backend.calls.some((c) => route(c) === `GET /videos/${VIDEO_KEY}/subtitles`), { timeout: 20_000 })
    .toBe(true);
}

test('test_overlay_follows_seek_and_pause', async () => {
  const p = makePlayer(MAIN_MS / 1000);
  const backend = readyBackend();
  await mountOverlay({ video: p.video, pageUrl: YT, title: TITLE, mediaUrl: null, request: backend.request });
  await subtitlesLoaded(backend);

  p.play(1.5);
  await expect.poll(shownSubtitle, SHOWN).toBe(CUES[0].text_ko);
  p.tick(4.0);
  await expect.poll(shownSubtitle, SHOWN).toBe('');

  p.seek(61);
  await expect.poll(shownSubtitle, SHOWN).toBe(CUES[2].text_ko);
  p.pause();
  await sleep(300);
  expect(shownSubtitle()).toBe(CUES[2].text_ko);
  p.play(61.2);
  await expect.poll(shownSubtitle, SHOWN).toBe(CUES[2].text_ko);
  p.tick(70.5);
  await expect.poll(shownSubtitle, SHOWN).toBe(CUES[3].text_ko);

  p.pause();
  p.seek(6);
  p.play(6.0);
  await expect.poll(shownSubtitle, SHOWN).toBe(CUES[1].text_ko);
  expect(p.writes).toEqual([]);
});

test('a fifteen-minute timeline and repeated backward seeks keep the source clock', async () => {
  const p = makePlayer(900);
  const cues = Array.from({ length: 15 }, (_, minute) => ({
    seq: minute, start_ms: (minute * 60 + 1) * 1000, end_ms: (minute * 60 + 5) * 1000, text_ko: `${minute}분 기준 자막`,
  }));
  const backend = fakeBackend((call) => route(call) === 'GET /jobs'
    ? { status: 200, body: job('ready', 100) }
    : { status: 200, body: { ...subtitlesBody(cues), duration_ms: 900_000 } });
  await mountOverlay({ video: p.video, pageUrl: YT, title: TITLE, mediaUrl: null, request: backend.request });
  await subtitlesLoaded(backend);
  for (const minute of [...Array.from({ length: 15 }, (_, i) => i), 0, 12, 3, 14, 1]) {
    p.seek(minute * 60 + 2);
    p.play(minute * 60 + 2);
    await expect.poll(shownSubtitle, SHOWN).toBe(cues[minute].text_ko);
    p.pause();
  }
  expect(p.writes).toEqual([]);
});

test('test_overlay_fits_phone_and_desktop_screens', async () => {
  const sequences: string[][] = [];
  for (const [w, h] of [[384, 854], [1280, 720]]) {
    await page.viewport(w, h);
    expect([innerWidth, innerHeight]).toEqual([w, h]);
    const p = makePlayer(MAIN_MS / 1000);
    let polls = 0;
    let created = false;
    let stage = '';
    const steps = [job('downloading', 0), job('writing', 40), job('translating', 60), job('ready', 100)];
    const backend = fakeBackend((c) => {
      switch (route(c)) {
        case 'POST /jobs':
          created = true;
          return { status: 200, body: { job_id: 'job-0001', video_key: VIDEO_KEY, stage: 'downloading', reused: false } };
        case 'GET /jobs':
          if (!created) return { status: 404, body: { error: 'no job' } };
          const step = steps[Math.min(polls++, steps.length - 1)];
          stage = step.stage;
          return { status: 200, body: step };
        case `GET /videos/${VIDEO_KEY}/subtitles`:
          return stage === 'ready'
            ? { status: 200, body: subtitlesBody([LONG_CUE]) }
            : { status: 409, body: { error: 'not ready' } };
        default:
          return { status: 404, body: { error: 'not found' } };
      }
    });
    const handle = await mountOverlay({ video: p.video, pageUrl: YT, title: TITLE, mediaUrl: null, request: backend.request });

    await expect.poll(() => deepAll('[data-vto="detect"]').filter(visible).length, { timeout: 5_000 }).toBe(1);
    const button = deepAll('[data-vto="detect"]').filter(visible)[0] as HTMLElement;
    expectOnScreen(button, `detect button at ${w}x${h}`);
    button.click();

    await expect.poll(() => backend.calls.some((c) => route(c) === 'POST /jobs'), { timeout: 5_000 }).toBe(true);
    const post = backend.calls.find((c) => route(c) === 'POST /jobs')!;
    expect(post.body).toMatchObject({ page_url: YT, title: TITLE, duration_ms: MAIN_MS });
    await subtitlesLoaded(backend);

    p.play(3.0);
    await expect.poll(shownSubtitle, SHOWN).toBe(LONG_CUE.text_ko);
    const box = deepAll('[data-vto="subtitle"]').filter(visible)[0];
    expectOnScreen(box, `subtitle at ${w}x${h}`);
    for (const b of deepAll('[data-vto="detect"]').filter(visible)) expectOnScreen(b, `detect button after ready at ${w}x${h}`);

    // The calls up to the first subtitle download, repeats collapsed.
    const all = backend.calls.map(route);
    const upto = all.slice(0, all.indexOf(`GET /videos/${VIDEO_KEY}/subtitles`) + 1);
    sequences.push(upto.filter((r, i) => i === 0 || upto[i - 1] !== r));
    handle.destroy();
  }
  expect(sequences[0]).toEqual(sequences[1]);
  const steps = sequences[0];
  const post = steps.indexOf('POST /jobs');
  expect(post, `no POST /jobs in ${steps}`).toBeGreaterThanOrEqual(0);
  expect(steps.indexOf('GET /jobs', post), `no polling after POST /jobs in ${steps}`).toBeGreaterThan(post);
  expect(steps.lastIndexOf(`GET /videos/${VIDEO_KEY}/subtitles`)).toBeGreaterThan(steps.indexOf('GET /jobs', post));
});

test('test_subtitles_hide_during_ad_and_return', async () => {
  const p = makePlayer(MAIN_MS / 1000);
  const backend = readyBackend();
  await mountOverlay({ video: p.video, pageUrl: YT, title: TITLE, mediaUrl: null, request: backend.request });
  await subtitlesLoaded(backend);

  p.play(61);
  await expect.poll(shownSubtitle, SHOWN).toBe(CUES[2].text_ko);

  // A 15 s ad takes over the same <video>, as on YouTube.
  let skipClicks = 0;
  const skip = document.createElement('button');
  skip.className = 'ytp-ad-skip-button-modern ytp-button';
  skip.textContent = 'Skip';
  skip.addEventListener('click', () => skipClicks++);
  p.player.classList.add('ad-showing', 'ad-interrupting');
  p.player.append(skip);
  p.load(15);
  p.play(1.5); // 1.5 s and 6 s fall inside lecture cues: an ad must not show them
  await expect.poll(shownSubtitle, SHOWN).toBe('');
  for (const t of [2.0, 2.5, 6.0, 7.0]) {
    p.tick(t);
    await sleep(150);
    expect(shownSubtitle(), `subtitle during the ad at ${t}s`).toBe('');
  }

  // The ad ends and the lecture resumes where it was.
  p.player.classList.remove('ad-showing', 'ad-interrupting');
  skip.remove();
  p.load(MAIN_MS / 1000);
  p.play(61.5);
  await expect.poll(shownSubtitle, SHOWN).toBe(CUES[2].text_ko);
  p.tick(70.5);
  await expect.poll(shownSubtitle, SHOWN).toBe(CUES[3].text_ko);

  expect(skipClicks).toBe(0);
  expect(p.writes).toEqual([]);
});
