// What the injected script does on a page, in every frame of it. One overlay
// per page, on its largest video, whether that is in the page itself or in an
// iframe (an embedded player). Frames report their video to the top frame by
// postMessage; the top picks the largest and grants that frame the overlay.
// A frame's overlay asks P6-P8 through the top frame, which owns the page: the
// job is made for the page's address and title (a YouTube embed keeps its own
// address, which names the video), and only the top frame reaches the app.

import { mountOverlay, videoMediaUrl, type Reply, type Request } from './index';

const TAG = 'vto-frames-1';
/** How long a frame's report stands without another. */
const STALE_MS = 3_000;
/** A job is made before POST /jobs answers; the media probe can take a while. */
const REQUEST_MS = 120_000;

/** A rectangle in a window's viewport coordinates, in px. */
export type Rect = { left: number; top: number; right: number; bottom: number };
const EVERYWHERE: Rect = { left: -Infinity, top: -Infinity, right: Infinity, bottom: Infinity };
const NOWHERE: Rect = { left: 0, top: 0, right: 0, bottom: 0 };

const cut = (a: Rect, b: Rect): Rect => ({
  left: Math.max(a.left, b.left),
  top: Math.max(a.top, b.top),
  right: Math.min(a.right, b.right),
  bottom: Math.min(a.bottom, b.bottom),
});
const areaOf = (r: Rect) => (r.right > r.left && r.bottom > r.top ? (r.right - r.left) * (r.bottom - r.top) : 0);
const viewportOf = (w: Window): Rect => ({ left: 0, top: 0, right: w.innerWidth, bottom: w.innerHeight });

/**
 * The document's main video: the one with the most of it on screen (inside
 * `clip`, the part of this frame the page shows). When none is on screen (the
 * page is scrolled past its player), the largest shown one, so the overlay
 * stays at the bottom of the screen for it.
 */
export function mainVideo(doc: Document = document, clip: Rect = EVERYWHERE): HTMLVideoElement | null {
  let best: HTMLVideoElement | null = null;
  let most = [0, 0];
  doc.querySelectorAll('video').forEach((v) => {
    if (!shown(v)) return;
    const r = v.getBoundingClientRect();
    const a = [visibleArea(v, clip), r.width * r.height];
    if (a[0] > most[0] || (a[0] === most[0] && a[1] > most[1])) [most, best] = [a, v];
  });
  return best;
}

/** Not hidden by CSS (display, visibility, opacity). */
function shown(e: Element): boolean {
  return e.checkVisibility?.({ opacityProperty: true, visibilityProperty: true }) ?? true;
}

/** How much of an element is inside its window's viewport and `clip`, in px² (0 when hidden). */
export function visibleArea(e: Element, clip: Rect = EVERYWHERE): number {
  return shown(e) ? areaOf(cut(cut(e.getBoundingClientRect(), viewportOf(e.ownerDocument.defaultView ?? window)), clip)) : 0;
}

/**
 * The part of an iframe's page the page around it shows, in that page's own
 * coordinates: its content box, cut by this window's viewport and by `clip`.
 * ponytail: CSS transforms (scale, rotate) on frames are not followed.
 */
function frameClip(f: HTMLIFrameElement, clip: Rect): Rect {
  if (!shown(f)) return NOWHERE;
  const r = f.getBoundingClientRect();
  const cs = getComputedStyle(f);
  const [pl, pt, pr, pb] = [cs.paddingLeft, cs.paddingTop, cs.paddingRight, cs.paddingBottom].map(parseFloat);
  // Where the frame's page starts: past the border (clientLeft/Top) and the padding.
  const x = r.left + f.clientLeft + pl;
  const y = r.top + f.clientTop + pt;
  const content = { left: x, top: y, right: x + f.clientWidth - pl - pr, bottom: y + f.clientHeight - pt - pb };
  const seen = cut(cut(content, viewportOf(window)), clip);
  if (!areaOf(seen)) return NOWHERE;
  return { left: seen.left - x, top: seen.top - y, right: seen.right - x, bottom: seen.bottom - y };
}

/** A rectangle from a message, or nothing seen when it is not one. */
function rectFrom(raw: any): Rect {
  const n = (v: unknown) => (typeof v === 'number' && Number.isFinite(v) ? v : NaN);
  const r = { left: n(raw?.left), top: n(raw?.top), right: n(raw?.right), bottom: n(raw?.bottom) };
  return Object.values(r).some(Number.isNaN) ? NOWHERE : r;
}

export function pageTitle(): string {
  const og = document.querySelector<HTMLMetaElement>('meta[property="og:title"]')?.content;
  return (og || document.title).replace(/\s+-\s+YouTube$/, '').trim();
}

function isYoutube(raw: unknown): boolean {
  try {
    const h = new URL(String(raw)).hostname;
    return /(^|\.)(youtube|youtube-nocookie)\.com$/.test(h) || h === 'youtu.be';
  } catch {
    return false;
  }
}

export interface PageOptions {
  /** `null` in the top frame; in an iframe, the top frame to report to. */
  top: Window | null;
  /** P6-P8 (the top frame's; an iframe asks the top instead). */
  request: Request;
}

export function startPage({ top, request }: PageOptions): { stop(): void } {
  // WebView can inject the bundle twice; both instances must share one subtitle cache.
  const owner = window as Window & { __vtoPage?: { stop(): void } };
  if (owner.__vtoPage) return owner.__vtoPage;
  let mounted: { destroy(): void; video: HTMLVideoElement; url: string } | null = null;
  // The playlist a blob: player fetched is looked up in Resource Timing; keep more
  // than the default 250 entries so a busy page still has it.
  performance.setResourceTimingBufferSize?.(5_000);
  // When the page URL last changed: playlists fetched before belong to an earlier page.
  let url = location.href;
  let since = 0;

  const send = (to: Window, m: object) => to.postMessage({ [TAG]: true, ...m }, '*');
  // Top frame: the frames with a video, and the one granted the overlay.
  const frames = new Map<Window, { area: number; at: number }>();
  let granted: Window | null = null;
  // An iframe: whether the top chose its video, and its requests awaiting a reply.
  let chosen = false;
  // The part of this frame the page shows, through every frame above it, in this
  // frame's coordinates, as its parent last said. A frame reports to the top
  // directly, and the top sees only its own iframes, so each frame tells its own
  // iframes theirs. Nothing until the parent has said.
  let clip: Rect = top ? NOWHERE : EVERYWHERE;
  const pending = new Map<number, (r: Reply) => void>();
  let nextId = 1;

  const viaTop: Request = (method, path, body) =>
    new Promise((resolve) => {
      const id = nextId++;
      const timer = setTimeout(() => {
        pending.delete(id);
        resolve({ status: 503, body: { error: 'no_page', message: '영상이 있는 페이지와 연결되지 않았습니다.' } });
      }, REQUEST_MS);
      pending.set(id, (r) => {
        clearTimeout(timer);
        resolve(r);
      });
      send(top!, { kind: 'request', id, method, path, body });
    });

  const onMessage = async (e: MessageEvent) => {
    const m = e.data;
    if (!m || m[TAG] !== true || !e.source) return;
    const from = e.source as Window;
    if (m.kind === 'view') {
      if (top && from === window.parent) clip = rectFrom(m.rect);
    } else if (!top) {
      if (m.kind === 'video') frames.set(from, { area: Number(m.area) || 0, at: performance.now() });
      else if (m.kind === 'request' && from === granted) {
        let body = m.body;
        if (m.path === '/jobs' && body && !isYoutube(body.page_url)) {
          body = { ...body, page_url: location.href, ...(m.method === 'POST' ? { title: pageTitle() || body.title } : {}) };
        }
        const reply = await request(m.method, m.path, body);
        send(from, { kind: 'reply', id: m.id, reply });
      }
    } else if (from === top) {
      if (m.kind === 'grant') chosen = m.yes === true;
      else if (m.kind === 'reply') {
        pending.get(m.id)?.(m.reply);
        pending.delete(m.id);
      }
    }
  };
  window.addEventListener('message', onMessage);

  // Video sites swap pages and players without a reload: re-mount when either changes.
  const check = () => {
    if (url !== location.href) {
      url = location.href;
      // Seen up to one check (a second) after it happened.
      since = Math.max(0, performance.now() - 1_000);
    }
    const video = mainVideo(document, clip);
    const own = video ? visibleArea(video, clip) : 0;
    document.querySelectorAll('iframe').forEach((f) => {
      if (f.contentWindow) send(f.contentWindow, { kind: 'view', rect: frameClip(f, clip) });
    });
    let want = video;
    if (!top) {
      // The largest video wins, here or in a frame (ties stay here).
      const now = performance.now();
      let best: Window | null = null;
      let most = own;
      for (const [w, f] of frames) {
        if (now - f.at > STALE_MS || w.closed) frames.delete(w);
        else if (f.area > most) [best, most] = [w, f.area];
      }
      if (granted && granted !== best) send(granted, { kind: 'grant', yes: false });
      granted = best;
      // Sent on every check: a frame that reloaded learns it again.
      if (granted) {
        send(granted, { kind: 'grant', yes: true });
        want = null;
      }
    } else {
      if (own > 0 || chosen) send(top, { kind: 'video', area: own });
      if (!chosen) want = null;
    }
    if (mounted && mounted.video === want && mounted.url === location.href) return;
    mounted?.destroy();
    mounted = null;
    if (!want || !document.body) return;
    const v = want;
    const from = since;
    const o = mountOverlay({
      video: v,
      pageUrl: location.href,
      title: pageTitle(),
      mediaUrl: () => videoMediaUrl(v, from),
      request: top ? viaTop : request,
    });
    mounted = { ...o, video: v, url: location.href };
  };
  const interval = setInterval(check, 1_000);
  check();

  const handle = {
    stop() {
      clearInterval(interval);
      window.removeEventListener('message', onMessage);
      mounted?.destroy();
      mounted = null;
      if (owner.__vtoPage === handle) delete owner.__vtoPage;
    },
  };
  owner.__vtoPage = handle;
  return handle;
}
