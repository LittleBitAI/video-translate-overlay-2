// The API of docs/design/api.yaml as the screens call it: Tauri's IPC in the
// app's window, HTTP to `vto-web` in a browser (a GET's arguments in the query).
// Every answer is `{status, body}`; an error body is `{error, message}`.

export type Reply = { status: number; body: any };

type Invoke = (cmd: string, args?: unknown) => Promise<unknown>;

const COMMANDS: Record<string, string> = {
  'POST /browser/open': 'browser_open',
  'GET /browser': 'get_browser',
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
  const args = subs ? { video_key: decodeURIComponent(subs[1]) } : (body ?? {});
  try {
    return { status: 200, body: await call(cmd, args) };
  } catch (e: any) {
    // Commands reject with the API's error body, which carries its HTTP status.
    return { status: typeof e?.status === 'number' ? e.status : 500, body: typeof e === 'object' && e ? e : { message: String(e) } };
  }
}

/** Won with at most one decimal, as the cost is shown. */
export function won(krw: number | null | undefined): string {
  return `${(krw ?? 0).toLocaleString('ko-KR', { maximumFractionDigits: 1 })}원`;
}
