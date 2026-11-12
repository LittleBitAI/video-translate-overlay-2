// The app's initialization script for site pages: mounts the overlay on the
// page's main <video> and bridges its requests to the Tauri commands P6-P8.
// Built into one IIFE by `vite build -c vite.overlay.config.ts` (the app's build.rs).

import { mountOverlay, type Reply, type Request } from './index';

type Invoke = (cmd: string, args?: unknown) => Promise<unknown>;

/** Google's sign-in pages never get the overlay (R23). */
const SIGN_IN_HOSTS = ['accounts.google.com', 'accounts.youtube.com'];

const invoke = (): Invoke | undefined => (window as any).__TAURI_INTERNALS__?.invoke;

export const request: Request = async (method, path, body) => {
  let cmd: string;
  let args: unknown = body ?? {};
  const subs = /^\/videos\/([^/]+)\/subtitles$/.exec(path);
  if (method === 'POST' && path === '/jobs') cmd = 'create_job';
  else if (method === 'GET' && path === '/jobs') cmd = 'get_job';
  else if (method === 'GET' && subs) {
    cmd = 'get_subtitles';
    args = { video_key: decodeURIComponent(subs[1]) };
  } else return { status: 404, body: { error: 'no_route', message: `${method} ${path}` } };
  const call = invoke();
  if (!call) return { status: 503, body: { error: 'no_app', message: '앱 안에서만 쓸 수 있습니다.' } };
  try {
    return { status: 200, body: await call(cmd, args) };
  } catch (e: any) {
    // Commands reject with the API's error body, which carries its HTTP status.
    return { status: typeof e?.status === 'number' ? e.status : 403, body: e } as Reply;
  }
};

/** The page's main video: the largest one on screen. */
function mainVideo(): HTMLVideoElement | null {
  let best: HTMLVideoElement | null = null;
  let area = 0;
  document.querySelectorAll('video').forEach((v) => {
    const r = v.getBoundingClientRect();
    if (r.width * r.height > area) {
      area = r.width * r.height;
      best = v;
    }
  });
  return best;
}

function title(): string {
  const og = document.querySelector<HTMLMetaElement>('meta[property="og:title"]')?.content;
  return (og || document.title).replace(/\s+-\s+YouTube$/, '').trim();
}

function start() {
  if (window.top !== window || SIGN_IN_HOSTS.includes(location.hostname)) return;
  let mounted: { destroy(): void; video: HTMLVideoElement; url: string } | null = null;
  // Video sites swap pages and players without a reload: re-mount when either changes.
  const check = () => {
    const video = mainVideo();
    if (mounted && mounted.video === video && mounted.url === location.href) return;
    mounted?.destroy();
    mounted = null;
    if (!video || !document.body) return;
    const src = video.currentSrc;
    const mediaUrl = /^https?:/.test(src) ? src : null; // blob: players are resolved by the backend
    const o = mountOverlay({ video, pageUrl: location.href, title: title(), mediaUrl, request });
    mounted = { ...o, video, url: location.href };
  };
  setInterval(check, 1_000);
  check();
}

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start);
else start();
