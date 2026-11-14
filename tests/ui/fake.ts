// A stand-in for `vto-web` behind `fetch`, and helpers to mount the screens.
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, vi } from 'vitest';
import App from '../../src/ui/App.svelte';
import { go } from '../../src/ui/router.svelte';

export type Call = { method: string; path: string; query: Record<string, string>; body: any };
/** `[status, body]`, or `'hang'` for an answer that never comes. */
export type Answer = [number, unknown] | 'hang';

export const calls: Call[] = [];

/** Serves the API through `handler` until the test ends. */
export function api(handler: (c: Call) => Answer | Promise<Answer>) {
  calls.length = 0;
  vi.stubGlobal('fetch', async (url: string, init: RequestInit = {}) => {
    const u = new URL(url, location.origin);
    const c: Call = {
      method: init.method ?? 'GET',
      path: u.pathname,
      query: Object.fromEntries(u.searchParams),
      body: init.body ? JSON.parse(String(init.body)) : undefined,
    };
    calls.push(c);
    const a = await handler(c);
    if (a === 'hang') return new Promise(() => {});
    return new Response(JSON.stringify(a[1]), { status: a[0], headers: { 'Content-Type': 'application/json' } });
  });
}

let shown: ReturnType<typeof mount> | null = null;

/** Mounts the app at `path`. */
export function open(path: '/' | '/preferences' | '/library') {
  go(path);
  const target = document.createElement('div');
  document.body.append(target);
  shown = mount(App, { target });
  flushSync();
  return target;
}

afterEach(() => {
  if (shown) unmount(shown);
  shown = null;
  document.body.innerHTML = '';
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

export const $ = (root: ParentNode, sel: string) => root.querySelector<HTMLElement>(sel);
export const $$ = (root: ParentNode, sel: string) => [...root.querySelectorAll<HTMLElement>(sel)];
export const text = (root: Element) => root.textContent?.replace(/\s+/g, ' ').trim() ?? '';
export const features = (root: ParentNode) => [...new Set($$(root, '[data-feature]').map((e) => e.dataset.feature))].sort();

export function button(root: ParentNode, label: string): HTMLButtonElement {
  const b = $$(root, 'button, a').find((e) => text(e) === label);
  if (!b) throw new Error(`no button "${label}" in: ${text(root as Element)}`);
  return b as HTMLButtonElement;
}

export function type(input: HTMLElement | null, value: string) {
  const i = input as HTMLInputElement;
  i.value = value;
  i.dispatchEvent(new Event('input', { bubbles: true }));
}
