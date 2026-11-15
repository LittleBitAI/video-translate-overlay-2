// The API of docs/design/api.yaml as the screens call it: Tauri's IPC in the
// app's window, HTTP to `vto-web` in a browser (a GET's arguments in the query).
// Every answer is `{status, body}`; an error body is `{error, message}`.

export type Reply = { status: number; body: any };

type Invoke = (cmd: string, args?: unknown) => Promise<unknown>;

const COMMANDS: Record<string, string> = {
  'GET /account': 'get_google_account',
  'POST /account/sign-in': 'google_sign_in',
  'POST /account/sign-out': 'google_sign_out',
  'POST /updates/check': 'check_updates',
  'POST /updates/dismiss': 'dismiss_update',
  'POST /updates/open': 'open_update',
  'POST /browser/open': 'browser_open',
  'GET /browser': 'get_browser',
  'GET /library/data': 'get_library',
  'POST /library/data': 'library_action',
  'DELETE /browser/site-data': 'clear_site_data',
  'GET /settings': 'get_settings',
  'PUT /settings/api-key': 'put_api_key',
  'DELETE /settings/api-key': 'delete_api_key',
  'POST /jobs': 'create_job',
  'GET /jobs': 'get_job',
};

const invoke = (): Invoke | undefined => (globalThis as any).__TAURI_INTERNALS__?.invoke;

/** True in the app's own window, where sites open in the app's webview. */
export const inApp = (): boolean => !!invoke();

const UNREACHABLE: Reply = { status: 0, body: { error: 'unreachable', message: '앱에 닿지 않습니다. 잠시 뒤에 다시 해 주세요.' } };

const LIBRARY_COMMANDS = new Set(['browser_open', 'get_browser', 'get_library', 'library_action', 'create_job', 'get_job', 'get_subtitles']);
let accountGeneration: number | undefined;
let accountEpoch = 0;
window.addEventListener('vto-account-changed', () => { accountGeneration = undefined; accountEpoch++; });
const ACCOUNT_CHANGED: Reply = { status: 409, body: { error: 'account_changed', message: '계정 상태가 바뀌었습니다. 화면을 다시 불러와 주세요.' } };

export async function request(method: string, path: string, body?: Record<string, unknown>): Promise<Reply> {
  const call = invoke();
  if (call) return viaIpc(call, method, path, body);
  let url = path;
  const init: RequestInit = { method };
  if (body && method === 'GET') {
    const q = new URLSearchParams();
    for (const [k, v] of Object.entries(body)) if (v != null) q.set(k, String(v));
    url += `?${q}`;
  } else if (body) {
    init.body = JSON.stringify(body);
    init.headers = { 'Content-Type': 'application/json' };
  }
  let r: Response;
  try {
    r = await fetch(url, init);
  } catch {
    return UNREACHABLE;
  }
  let parsed: any = null;
  try {
    parsed = await r.json();
  } catch {
    // An error page without a JSON body: the status says enough.
  }
  return { status: r.status, body: parsed ?? {} };
}

async function viaIpc(call: Invoke, method: string, path: string, body?: Record<string, unknown>): Promise<Reply> {
  const subs = /^\/videos\/([^/]+)\/subtitles$/.exec(path);
  const cmd = subs && method === 'GET' ? 'get_subtitles' : COMMANDS[`${method} ${path}`];
  if (!cmd) return { status: 404, body: { error: 'no_route', message: `${method} ${path}` } };
  let args: Record<string, unknown> = subs ? { video_key: decodeURIComponent(subs[1]) } : path === '/library/data' && method === 'POST' ? { action: body } : (body ?? {});
  const scoped = LIBRARY_COMMANDS.has(cmd);
  const epoch = accountEpoch;
  try {
    if (scoped) {
      const status: any = await call('get_google_account', {});
      if (epoch !== accountEpoch) return ACCOUNT_CHANGED;
      if (!Number.isSafeInteger(status?.generation)) return ACCOUNT_CHANGED;
      if (accountGeneration !== undefined && accountGeneration !== status.generation) {
        window.dispatchEvent(new CustomEvent('vto-account-changed'));
        return ACCOUNT_CHANGED;
      }
      accountGeneration = status.generation;
      args = { ...args, account_generation: body?.account_generation ?? accountGeneration };
    }
    const result = await call(cmd, args);
    if (scoped && epoch !== accountEpoch) return ACCOUNT_CHANGED;
    return { status: 200, body: result };
  } catch (e: any) {
    if (scoped && epoch !== accountEpoch) return ACCOUNT_CHANGED;
    // Commands reject with the API's error body, which carries its HTTP status.
    return { status: typeof e?.status === 'number' ? e.status : 500, body: typeof e === 'object' && e ? e : { message: String(e) } };
  }
}

/** Won with at most one decimal, as the cost is shown. */
export function won(krw: number | null | undefined): string {
  return `${(krw ?? 0).toLocaleString('ko-KR', { maximumFractionDigits: 1 })}원`;
}
