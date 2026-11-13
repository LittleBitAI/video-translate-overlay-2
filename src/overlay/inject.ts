// The app's initialization script for site pages, run in every frame: mounts
// the overlay on the page's main <video> (`page.ts`) and bridges its requests
// to the Tauri commands P6-P8. Built into one IIFE by
// `vite build -c vite.overlay.config.ts` (the app's build.rs).

import type { Reply, Request } from './index';
import { startPage } from './page';
import { signInHelp } from './signin';

type Invoke = (cmd: string, args?: unknown) => Promise<unknown>;

/** Google's sign-in pages never get the overlay, only the skip button (R22, R23). */
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

function start() {
  const top = window.top === window ? null : window.top;
  if (SIGN_IN_HOSTS.includes(location.hostname)) {
    if (!top) signInHelp({ store: sessionStorage, go: (u) => location.assign(u) });
    return;
  }
  startPage({ top, request });
}

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start);
else start();
