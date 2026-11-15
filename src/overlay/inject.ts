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
let owner: number | undefined;
let retired = false;
window.addEventListener('vto-account-changed', () => { retired = true; });

export const request: Request = async (method, path, body) => {
  let cmd: string;
  let args: Record<string, unknown> = body ?? {};
  const subs = /^\/videos\/([^/]+)\/subtitles$/.exec(path);
  if (method === 'POST' && path === '/jobs') cmd = 'create_job';
  else if (method === 'GET' && path === '/jobs') cmd = 'get_job';
  else if (method === 'GET' && subs) {
    cmd = 'get_subtitles';
    args = { video_key: decodeURIComponent(subs[1]) };
  } else return { status: 404, body: { error: 'no_route', message: `${method} ${path}` } };
  const call = invoke();
  if (!call) return { status: 503, body: { error: 'no_app', message: '앱 안에서만 쓸 수 있습니다.' } };
  const changed: Reply = { status: 409, body: { error:'account_changed', message:'계정 상태가 바뀌었습니다. 앱에서 영상을 다시 열어 주세요.' } };
  if (retired) return changed;
  try {
    const result = await call(cmd, {...args, account_generation:owner});
    return retired ? changed : { status: 200, body: result };
  } catch (e: any) {
    if (retired) return changed;
    if (owner === undefined && cmd === 'get_job' && e?.error === 'account_required' && Number.isSafeInteger(e.account_generation)) {
      owner = e.account_generation;
      return request(method, path, body);
    }
    if (e?.error === 'account_changed') {
      retired = true;
      window.dispatchEvent(new Event('vto-account-changed'));
    }
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
