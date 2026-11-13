// The overlay the in-app browser attaches to video pages: the 영상감지 button,
// the job's progress while it runs, and the Korean subtitle of the moment.
//
// It only reads the player (currentTime, duration, paused); it never seeks,
// changes speed, plays, pauses or touches ad controls. Cues are on the main
// video's timeline, so while an ad plays (another duration, or YouTube's
// `ad-showing` class) nothing is shown and the lecture's cue returns with it.

export type Reply = { status: number; body: any };
export type Request = (method: string, path: string, body?: unknown) => Promise<Reply>;
export type Cue = { seq: number; start_ms: number; end_ms: number; text_ko: string };

export interface OverlayOptions {
  video: HTMLVideoElement;
  pageUrl: string;
  title: string;
  /** A function is read when 영상감지 is pressed: sites often set the media after the player appears. */
  mediaUrl: string | null | (() => string | null | Promise<string | null>);
  /** P6, P7 and P8 of docs/design/api.yaml (Tauri `invoke` in the app). */
  request: Request;
}

const POLL_MS = 1_000;
const AD_TOLERANCE_MS = 1_000;

const STAGE_KO: Record<string, string> = {
  downloading: '오디오 내려받는 중',
  writing: '말을 글로 옮기는 중',
  translating: '한국어로 번역하는 중',
  ready: '자막 준비 완료',
  failed: '작업 실패',
};

// The five `failure_reason` values (E2), plus the 428 reply's missing key.
const FAILURE_KO: Record<string, string> = {
  download_failed: '오디오를 내려받거나 읽지 못했습니다.',
  no_speech: '영상에서 말소리를 찾지 못했습니다.',
  key_rejected: 'Gemini가 API 키를 받아 주지 않았습니다.',
  cost_limit: '비용 한도(1시간에 1,000원)를 넘어 번역을 멈췄습니다.',
  translate_failed: '번역하지 못했습니다. 네트워크를 확인하고 다시 눌러 주세요.',
  no_key: '설정에서 Gemini API 키를 넣어 주세요.',
};

// The root lies over the video's box (the part of it on screen): the button at
// its top left, the subtitle low in it, above where players put their controls.
// `--w`/`--h` are that box's size. A video with too little of it on screen gets
// the root across the bottom of the screen instead, so the button is never lost.
const CSS = `
:host { all: initial; }
.root { position: fixed; z-index: 2147483647; pointer-events: none; box-sizing: border-box;
  display: flex; flex-direction: column; justify-content: space-between; gap: 8px; padding: 8px;
  font-family: system-ui, "Noto Sans KR", sans-serif; }
.root.loose { justify-content: flex-end; align-items: center; padding: 0 16px; }
.sub { pointer-events: none; align-self: center; max-width: 100%; box-sizing: border-box; padding: 4px 10px; border-radius: 4px;
  margin-bottom: calc(var(--h, 0px) * 0.1);
  background: rgba(0, 0, 0, 0.78); color: #fff; font-size: clamp(14px, calc(var(--w, 100vw) * 0.03), 26px); line-height: 1.4;
  text-align: center; overflow-wrap: anywhere; word-break: keep-all; white-space: pre-wrap; }
.loose .sub { order: -1; margin-bottom: 0; }
.bar { pointer-events: auto; align-self: flex-start; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; max-width: 100%; }
.loose .bar { align-self: center; }
button { font: inherit; font-size: 14px; padding: 6px 12px; border: 0; border-radius: 16px; background: #e53935;
  color: #fff; cursor: pointer; flex: none; }
button:disabled { background: #757575; cursor: default; }
.status { font-size: 13px; color: #fff; background: rgba(0, 0, 0, 0.6); padding: 3px 8px; border-radius: 4px;
  overflow-wrap: anywhere; min-width: 0; }
.status:empty { display: none; }
`;

export function mountOverlay({ video, pageUrl, title, mediaUrl, request }: OverlayOptions): { destroy(): void } {
  const host = document.createElement('div');
  host.setAttribute('data-vto', 'overlay');
  // Inside a site's player (in full screen) its rules must not reach the host either.
  host.style.setProperty('all', 'initial', 'important');
  const shadow = host.attachShadow({ mode: 'open' });
  const style = el('style');
  style.textContent = CSS;
  shadow.append(style);
  const root = el('div', 'root');
  const sub = el('div', 'sub');
  sub.dataset.vto = 'subtitle';
  sub.dataset.feature = 'watch-subtitles';
  sub.hidden = true;
  const bar = el('div', 'bar');
  const button = el('button') as HTMLButtonElement;
  button.dataset.vto = 'detect';
  button.dataset.feature = 'detect-video';
  button.type = 'button';
  button.textContent = '영상감지';
  const fullscreen = el('button') as HTMLButtonElement;
  fullscreen.dataset.vto = 'fullscreen';
  fullscreen.type = 'button';
  fullscreen.textContent = '전체화면';
  fullscreen.setAttribute('aria-label', '영상과 자막 전체화면');
  const status = el('div', 'status');
  status.dataset.vto = 'status';
  status.dataset.feature = 'job-progress';
  status.setAttribute('role', 'status');
  bar.append(button, fullscreen, status);
  root.append(bar, sub);
  shadow.append(root);

  let placed = '';
  /** Over the video, inside the full-screen element when the player is in full screen. */
  const place = () => {
    const d = document as Document & { webkitFullscreenElement?: Element | null };
    const full = d.fullscreenElement ?? d.webkitFullscreenElement ?? null;
    fullscreen.textContent = full ? '화면복귀' : '전체화면';
    // Request the player container from our button: video-only fullscreen cannot paint sibling subtitles.
    const parent = full && full !== video && full.contains(video) ? full : document.body ?? document.documentElement;
    if (host.parentNode !== parent) parent.append(host);
    const r = video.getBoundingClientRect();
    const left = Math.max(0, r.left);
    const top = Math.max(0, r.top);
    const w = Math.min(innerWidth, r.right) - left;
    const h = Math.min(innerHeight, r.bottom) - top;
    const loose = !(w >= 160 && h >= 90);
    const key = loose ? 'loose' : `${left},${top},${w},${h}`;
    if (key === placed) return;
    placed = key;
    root.classList.toggle('loose', loose);
    const s = root.style;
    if (loose) {
      s.cssText = 'left:0;right:0;bottom:16px';
    } else {
      s.cssText = `left:${left}px;top:${top}px;width:${w}px;height:${h}px`;
      s.setProperty('--w', `${w}px`);
      s.setProperty('--h', `${h}px`);
    }
  };
  place();
  const placer = setInterval(place, 250);
  const resized = typeof ResizeObserver === 'function' ? new ResizeObserver(place) : null;
  resized?.observe(video);
  const moves: [EventTarget, string][] = [
    [window, 'resize'],
    [window, 'scroll'],
    [document, 'fullscreenchange'],
    [document, 'webkitfullscreenchange'],
  ];
  moves.forEach(([t, e]) => t.addEventListener(e, place, { capture: true, passive: true }));

  let cues: Cue[] = [];
  let ready = false;
  let mainMs = 0;
  let timer: ReturnType<typeof setInterval> | undefined;
  let ticker: ReturnType<typeof setInterval> | undefined;
  let destroyed = false;

  const adShowing = () => !!(video.closest('.ad-showing') || document.querySelector('.html5-video-player.ad-showing'));
  const isAd = () => {
    if (adShowing()) return true;
    const d = video.duration;
    return mainMs > 0 && Number.isFinite(d) && Math.abs(d * 1000 - mainMs) > AD_TOLERANCE_MS;
  };

  const render = () => {
    place();
    const t = video.currentTime * 1000;
    const cue = cues.length && !isAd() ? findCue(cues, t) : undefined;
    const text = cue?.text_ko ?? '';
    if (sub.textContent !== text) sub.textContent = text;
    sub.hidden = !text;
    // timeupdate fires only ~4 times a second; tick faster while playing so cues land on time.
    if (!video.paused && cues.length && !ticker) ticker = setInterval(render, 100);
    if ((video.paused || !cues.length) && ticker) {
      clearInterval(ticker);
      ticker = undefined;
    }
  };
  const events = ['timeupdate', 'seeked', 'seeking', 'play', 'playing', 'pause', 'loadedmetadata', 'durationchange', 'emptied'];
  events.forEach((e) => video.addEventListener(e, render));

  const say = (text: string) => {
    status.textContent = text;
  };

  fullscreen.addEventListener('click', async () => {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        const player = video.closest<HTMLElement>('.html5-video-player') ?? video.parentElement;
        if (!player?.requestFullscreen) throw new Error('Fullscreen unavailable');
        // Put both video and subtitles in the same fullscreen element.
        if (!player.contains(host)) player.append(host);
        await player.requestFullscreen();
      }
    } catch {
      say('전체화면을 열지 못했습니다. 다시 눌러 주세요.');
    }
  });

  const loadSubtitles = async (videoKey: string) => {
    const r = await request('GET', `/videos/${encodeURIComponent(videoKey)}/subtitles`);
    if (destroyed) return;
    if (r.status === 200) {
      cues = [...(r.body.cues as Cue[])].sort((a, b) => a.start_ms - b.start_ms);
      mainMs = r.body.duration_ms || 0;
      ready = true;
      button.textContent = '다시 번역';
      button.title = '저장된 원문과 시각을 유지해 다시 번역합니다. 추가 API 비용이 발생합니다.';
      say('');
      button.disabled = false;
      render();
    } else {
      say(r.body?.message ?? '자막을 아직 받을 수 없습니다.');
    }
  };

  const stopPolling = () => {
    if (timer) clearInterval(timer);
    timer = undefined;
  };

  /** Whether the first look at the job was made: one skipped while hidden is made on return. */
  let looked = false;

  /** One look at the job: shows its progress, or loads the subtitles once ready. */
  const poll = async () => {
    if (document.visibilityState === 'hidden') return;
    looked = true;
    const r = await request('GET', '/jobs', { page_url: pageUrl });
    if (destroyed || r.status !== 200) return;
    const j = r.body;
    if (j.stage === 'ready') {
      stopPolling();
      await loadSubtitles(j.video_key);
    } else if (j.stage === 'failed') {
      stopPolling();
      button.disabled = false;
      ready = false;
      button.textContent = '영상감지';
      await loadSubtitles(j.video_key);
      if (destroyed) return;
      say(`${STAGE_KO.failed}: ${FAILURE_KO[j.failure_reason] ?? j.failure_reason ?? ''}`);
    } else {
      button.disabled = true;
      say(`${STAGE_KO[j.stage] ?? j.stage} ${j.percent ?? 0}%`);
      if (!timer) timer = setInterval(poll, POLL_MS);
    }
  };

  button.addEventListener('click', async () => {
    button.disabled = true;
    if (ready) {
      say('저장된 원문을 다시 번역하는 중… (API 비용 발생)');
      const r = await request('POST', '/jobs', { page_url: pageUrl, retranslate: true });
      if (destroyed) return;
      if (r.status !== 200) {
        button.disabled = false;
        say(r.body?.message ?? '다시 번역을 시작하지 못했습니다.');
        return;
      }
      ready = false;
      if (!timer) timer = setInterval(poll, POLL_MS);
      await poll();
      return;
    }
    // While an ad plays, the player's length and media are the ad's: start once it is over.
    if (adShowing()) {
      say('광고가 끝나면 작업을 시작합니다.');
      // Then up to 10 s for the lecture's length to be known (the backend refuses an unknown one).
      let left = 10_000;
      while (adShowing() || (!(video.duration > 0) && (left -= 250) > 0)) {
        await new Promise((r) => setTimeout(r, 250));
        if (destroyed) return;
      }
    }
    say('작업을 만드는 중…');
    const durationMs = Number.isFinite(video.duration) ? Math.round(video.duration * 1000) : 0;
    const media = typeof mediaUrl === 'function' ? await mediaUrl() : mediaUrl;
    if (destroyed) return;
    const r = await request('POST', '/jobs', { page_url: pageUrl, media_url: media, title, duration_ms: durationMs });
    if (destroyed) return;
    if (r.status === 200) {
      if (r.body.stage === 'ready') {
        await loadSubtitles(r.body.video_key);
      } else {
        say(`${STAGE_KO[r.body.stage] ?? r.body.stage} 0%`);
        if (!timer) timer = setInterval(poll, POLL_MS);
      }
      return;
    }
    button.disabled = false;
    if (r.status === 428) say(FAILURE_KO.no_key);
    else if (r.status === 409) say('다른 영상의 작업이 돌고 있습니다. 끝난 뒤에 다시 눌러 주세요.');
    else say(r.body?.message ?? `작업을 시작하지 못했습니다 (${r.status}).`);
  });

  const onVisible = () => {
    if (document.visibilityState === 'visible' && (timer || !looked)) poll();
  };
  document.addEventListener('visibilitychange', onVisible);

  poll(); // a job may already exist for this page (made earlier, or still running)

  return {
    destroy() {
      destroyed = true;
      stopPolling();
      if (ticker) clearInterval(ticker);
      clearInterval(placer);
      resized?.disconnect();
      moves.forEach(([t, e]) => t.removeEventListener(e, place, { capture: true }));
      events.forEach((e) => video.removeEventListener(e, render));
      document.removeEventListener('visibilitychange', onVisible);
      host.remove();
    },
  };
}

function el(tag: string, cls?: string): HTMLElement {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  return e;
}

/**
 * The video's media as of now. A blob: (MediaSource) player such as hls.js plays
 * a playlist its script fetched; Resource Timing saw that fetch, so a playlist
 * the page loaded at or after `since` (when its URL last changed) stands in for
 * it: the one as long as the video (an ad's playlist loaded first is passed
 * over), and none when none is (an ad's audio must never become the
 * lecture's subtitles). The backend probes master playlists and their variants
 * itself. YouTube's player API is requested in the page's session; the native
 * worker downloads the resulting signed audio URL without needing the page.
 */
export async function videoMediaUrl(video: HTMLVideoElement, since = 0, pageUrl = location.href): Promise<string | null> {
  const page = new URL(pageUrl);
  if (page.hostname === 'youtube.com' || page.hostname.endsWith('.youtube.com')) {
    return youtubeAudioUrl(page, video.duration);
  }
  const src = video.currentSrc || video.src || video.querySelector('source')?.src || '';
  if (/^https?:/.test(src)) return src;
  if (!src.startsWith('blob:')) return null;
  const found = performance
    .getEntriesByType('resource')
    .filter((e) => e.startTime >= since && /^https?:/.test(e.name) && /\.(m3u8|mpd)$/i.test(new URL(e.name).pathname))
    .map((e) => e.name);
  const candidates = [...new Set(found)];
  if (!candidates.length) return null;
  const want = video.duration * 1000;
  // A live stream (or one not loaded yet): nothing to match; the backend refuses those itself.
  if (!(want > 0) || !Number.isFinite(want)) return candidates[0];
  const lengths = await Promise.all(candidates.map((u) => playlistMs(u).catch(() => null)));
  // ponytail: playlists this page cannot read again (CORS, one-time tokens) give no length;
  // when none gives one, the first is sent as before. Upgrade path: hook MediaSource.
  if (lengths.every((l) => l === null)) return candidates[0];
  const tolerance = Math.max(2_000, want * 0.02);
  let best: string | null = null;
  let off = Infinity;
  for (const [i, u] of candidates.entries()) {
    const l = lengths[i];
    if (l !== null && Math.abs(l - want) <= tolerance && Math.abs(l - want) < off) {
      best = u;
      off = Math.abs(l - want);
    }
  }
  return best;
}

/** Reuse the WebView session instead of making an anonymous native player request. */
async function youtubeAudioUrl(page: URL, duration: number): Promise<string | null> {
  const id = page.searchParams.get('v') ?? /^\/(?:shorts|embed|live)\/([\w-]{11})(?:\/|$)/.exec(page.pathname)?.[1];
  if (!id || !/^[\w-]{11}$/.test(id) || !Number.isFinite(duration) || duration <= 0) return null;
  try {
    // shortcut: this undocumented client can change; update it when YouTube stops serving audio.
    const client = {
      clientName: 'VISIONOS', clientVersion: '1.02', deviceMake: 'Apple', deviceModel: 'RealityDevice17,1',
      osName: 'visionOS', osVersion: '26.5.23O471', hl: 'en',
      userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15',
      visitorData: (window as any).ytcfg?.get?.('VISITOR_DATA'),
    };
    const response = await fetch('/youtubei/v1/player?prettyPrint=false', {
      method: 'POST', credentials: 'same-origin', signal: AbortSignal.timeout(15_000),
      headers: { 'Content-Type': 'application/json', 'X-YouTube-Client-Name': '101', 'X-YouTube-Client-Version': '1.02' },
      body: JSON.stringify({ videoId: id, context: { client }, contentCheckOk: true, racyCheckOk: true }),
    });
    if (!response.ok) return null;
    const player = await response.json();
    if (player.playabilityStatus?.status !== 'OK' || player.videoDetails?.videoId !== id
      || player.videoDetails?.isLive || player.videoDetails?.isUpcoming || player.playabilityStatus?.liveStreamability) return null;
    const formats = (player.streamingData?.adaptiveFormats ?? []).filter((f: any) =>
      typeof f.url === 'string' && f.mimeType?.startsWith('audio/') && f.audioTrack?.audioIsDefault !== false);
    formats.sort((a: any, b: any) => Number(b.mimeType.includes('mp4a')) - Number(a.mimeType.includes('mp4a'))
      || (b.bitrate ?? 0) - (a.bitrate ?? 0));
    for (const format of formats) {
      const u = new URL(format.url);
      const seconds = Number(u.searchParams.get('dur'));
      if (u.protocol === 'https:' && u.hostname.endsWith('.googlevideo.com') && u.pathname === '/videoplayback'
        && u.searchParams.get('source') === 'youtube' && u.searchParams.get('mime')?.startsWith('audio/')
        && Number.isFinite(seconds) && seconds > 0 && Math.abs(seconds - duration) <= Math.max(2, duration * 0.02)) return u.href;
    }
  } catch { /* A network/player failure falls back to the native extractor. */ }
  return null;
}

/** How long an HLS playlist (a master's first variant) or a DASH manifest plays, in ms. */
async function playlistMs(url: string, depth = 0): Promise<number | null> {
  // As players fetch it (hls.js and dash.js send no cookies to another origin).
  const r = await fetch(url);
  if (!r.ok) return null;
  const text = await r.text();
  if (/\.mpd$/i.test(new URL(url).pathname) || text.includes('<MPD')) {
    const m = /mediaPresentationDuration="P(?:(\d+(?:\.\d+)?)D)?T?(?:(\d+(?:\.\d+)?)H)?(?:(\d+(?:\.\d+)?)M)?(?:(\d+(?:\.\d+)?)S)?"/.exec(text);
    if (!m) return null;
    const [d, h, min, s] = m.slice(1).map((x) => Number(x ?? 0));
    return (((d * 24 + h) * 60 + min) * 60 + s) * 1000;
  }
  const lines = text.split(/\r?\n/).map((l) => l.trim());
  const variant = lines.findIndex((l) => l.startsWith('#EXT-X-STREAM-INF'));
  if (variant >= 0) {
    const uri = lines.slice(variant + 1).find((l) => l && !l.startsWith('#'));
    return uri && depth === 0 ? playlistMs(new URL(uri, url).href, 1) : null;
  }
  let ms = 0;
  for (const l of lines) {
    const m = /^#EXTINF:\s*([\d.]+)/.exec(l);
    if (m) ms += Number(m[1]) * 1000;
  }
  return ms > 0 ? ms : null;
}

/** The cue under `ms` (cues sorted by start, not overlapping). */
export function findCue(cues: Cue[], ms: number): Cue | undefined {
  let lo = 0;
  let hi = cues.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (cues[mid].start_ms <= ms) lo = mid + 1;
    else hi = mid - 1;
  }
  const c = cues[hi];
  return c && ms < c.end_ms ? c : undefined;
}
