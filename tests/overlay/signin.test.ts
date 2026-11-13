// On the YouTube account screen P1 opens first, the user can skip sign-in,
// and a refusal by Google (no sign-in inside apps) goes on to the video by
// itself (F2, R22). The button only ever leads to YouTube.

import { expect, onTestFinished, test } from 'vitest';
import { refused, signInHelp } from '../../src/overlay/signin';

const VIDEO = 'https://www.youtube.com/watch?v=signin0001';
const GATE = `https://accounts.google.com/ServiceLogin?service=youtube&continue=${encodeURIComponent(VIDEO)}`;

function memoryStore(): Storage {
  const m = new Map<string, string>();
  return {
    getItem: (k) => m.get(k) ?? null,
    setItem: (k, v) => void m.set(k, String(v)),
    removeItem: (k) => void m.delete(k),
    clear: () => m.clear(),
    key: (i) => [...m.keys()][i] ?? null,
    get length() {
      return m.size;
    },
  };
}

function skipButton(): HTMLButtonElement | null {
  const host = document.querySelector('[data-vto="sign-in"]');
  return (host?.shadowRoot?.querySelector('[data-vto="skip-sign-in"]') as HTMLButtonElement) ?? null;
}

function help(opts: Parameters<typeof signInHelp>[0]) {
  const h = signInHelp(opts);
  onTestFinished(() => h.destroy());
  return h;
}

test('test_skipping_sign_in_opens_the_video', () => {
  const opened: string[] = [];
  help({ store: memoryStore(), go: (u) => opened.push(u), href: () => GATE, text: () => 'Sign in to continue to YouTube' });
  const b = skipButton();
  expect(b).not.toBeNull();
  expect(b!.getBoundingClientRect().width).toBeGreaterThan(0);
  b!.click();
  expect(opened).toEqual([VIDEO]);
});

test('test_google_refusing_sign_in_goes_on_to_the_video', async () => {
  const store = memoryStore();
  const opened: string[] = [];
  // The first page keeps where to go; Google's refusal page no longer says it.
  help({ store, go: () => {}, href: () => GATE, text: () => '' }).destroy();
  help({ store, go: (u) => opened.push(u), href: () => 'https://accounts.google.com/v3/signin/rejected?TL=abc', text: () => '' });
  expect(opened).toEqual([VIDEO]);

  // Said only in the page's text, after the page changed without a reload.
  const later: string[] = [];
  let text = 'Sign in';
  help({ store: memoryStore(), go: (u) => later.push(u), href: () => GATE, text: () => text });
  expect(later).toEqual([]);
  text = "Couldn't sign you in. This browser or app may not be secure.";
  await expect.poll(() => later, { timeout: 3_000 }).toEqual([VIDEO]);

  expect(refused('https://accounts.google.com/signin/oauth/error?authError=x', '')).toBe(true);
  expect(refused('https://accounts.google.com/v3/signin/identifier', 'Wrong password')).toBe(false);
});

test('test_the_skip_button_never_leads_off_youtube', () => {
  const opened: string[] = [];
  const evil = `https://accounts.google.com/ServiceLogin?continue=${encodeURIComponent('https://evil.example/')}`;
  help({ store: memoryStore(), go: (u) => opened.push(u), href: () => evil, text: () => 'disallowed_useragent' });
  expect(skipButton()).toBeNull();
  expect(opened).toEqual([]);
});
