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
  mediaUrl: string | null;
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

const FAILURE_KO: Record<string, string> = {
  cost_limit: '비용 한도(1시간에 1,000원)를 넘어 번역을 멈췄습니다.',
  no_key: '설정에서 Gemini API 키를 넣어 주세요.',
  key_rejected: 'Gemini가 API 키를 받아 주지 않았습니다.',
  network: '네트워크에 닿지 않아 번역하지 못했습니다.',
  download_failed: '오디오를 내려받지 못했습니다.',
  decode_failed: '오디오를 읽지 못했습니다.',
  write_failed: '기기 안 모델이 글로 옮기지 못했습니다.',
};

const CSS = `
:host { all: initial; }
.root { position: fixed; left: 0; right: 0; bottom: 16px; z-index: 2147483647; pointer-events: none;
  display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 0 16px; box-sizing: border-box;
  font-family: system-ui, "Noto Sans KR", sans-serif; }
.sub { pointer-events: none; max-width: 100%; box-sizing: border-box; padding: 4px 10px; border-radius: 4px;
  background: rgba(0, 0, 0, 0.78); color: #fff; font-size: clamp(14px, 2.4vw, 22px); line-height: 1.4;
  text-align: center; overflow-wrap: anywhere; word-break: keep-all; white-space: pre-wrap; }
.bar { pointer-events: auto; display: flex; align-items: center; gap: 8px; max-width: 100%; }
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
  const shadow = host.attachShadow({ mode: 'open' });
  shadow.innerHTML = `<style>${CSS}</style>`;
  const root = el('div', 'root');
  const sub = el('div', 'sub');
  sub.dataset.vto = 'subtitle';
  sub.hidden = true;
  const bar = el('div', 'bar');
  const button = el('button') as HTMLButtonElement;
  button.dataset.vto = 'detect';
  button.type = 'button';
  button.textContent = '영상감지';
  const status = el('div', 'status');
  status.dataset.vto = 'status';
  status.setAttribute('role', 'status');
  bar.append(button, status);
  root.append(sub, bar);
  shadow.append(root);
  document.body.append(host);

  let cues: Cue[] = [];
  let mainMs = 0;
  let timer: ReturnType<typeof setInterval> | undefined;
  let ticker: ReturnType<typeof setInterval> | undefined;
  let destroyed = false;

  const isAd = () => {
    if (video.closest('.ad-showing') || document.querySelector('.html5-video-player.ad-showing')) return true;
    const d = video.duration;
    return mainMs > 0 && Number.isFinite(d) && Math.abs(d * 1000 - mainMs) > AD_TOLERANCE_MS;
  };

  const render = () => {
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

  const loadSubtitles = async (videoKey: string) => {
    const r = await request('GET', `/videos/${encodeURIComponent(videoKey)}/subtitles`);
    if (destroyed) return;
    if (r.status === 200) {
      cues = [...(r.body.cues as Cue[])].sort((a, b) => a.start_ms - b.start_ms);
      mainMs = r.body.duration_ms || 0;
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

  /** One look at the job: shows its progress, or loads the subtitles once ready. */
  const poll = async () => {
    if (document.visibilityState === 'hidden') return;
    const r = await request('GET', '/jobs', { page_url: pageUrl });
    if (destroyed || r.status !== 200) return;
    const j = r.body;
    if (j.stage === 'ready') {
      stopPolling();
      await loadSubtitles(j.video_key);
    } else if (j.stage === 'failed') {
      stopPolling();
      button.disabled = false;
      say(`${STAGE_KO.failed}: ${FAILURE_KO[j.failure_reason] ?? j.failure_reason ?? ''}`);
    } else {
      button.disabled = true;
      say(`${STAGE_KO[j.stage] ?? j.stage} ${j.percent ?? 0}%`);
      if (!timer) timer = setInterval(poll, POLL_MS);
    }
  };

  button.addEventListener('click', async () => {
    button.disabled = true;
    say('작업을 만드는 중…');
    const durationMs = Number.isFinite(video.duration) ? Math.round(video.duration * 1000) : 0;
    const r = await request('POST', '/jobs', { page_url: pageUrl, media_url: mediaUrl, title, duration_ms: durationMs });
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
    if (document.visibilityState === 'visible' && timer) poll();
  };
  document.addEventListener('visibilitychange', onVisible);

  poll(); // a job may already exist for this page (made earlier, or still running)

  return {
    destroy() {
      destroyed = true;
      stopPolling();
      if (ticker) clearInterval(ticker);
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
