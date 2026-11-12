// P1 takes http addresses, so the account screen's `continue` can be an http
// YouTube address: skipping sign-in, or Google refusing it, still goes on to
// the video, over https (R22). An http address elsewhere still leads nowhere.

import { expect, onTestFinished, test } from 'vitest';
import { signInHelp } from '../../src/overlay/signin';

const gate = (video: string) => `https://accounts.google.com/ServiceLogin?service=youtube&continue=${encodeURIComponent(video)}`;

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

test('an http YouTube address is skipped to over https', () => {
  const opened: string[] = [];
  const h = signInHelp({ store: memoryStore(), go: (u) => opened.push(u), href: () => gate('http://www.youtube.com/watch?v=abcDEF12345'), text: () => '' });
  onTestFinished(() => h.destroy());
  skipButton()!.click();
  expect(opened).toEqual(['https://www.youtube.com/watch?v=abcDEF12345']);
});

test('Google refusing sign-in goes on from an http YouTube address', () => {
  const store = memoryStore();
  signInHelp({ store, go: () => {}, href: () => gate('http://youtu.be/abcDEF12345'), text: () => '' }).destroy();
  const opened: string[] = [];
  const h = signInHelp({ store, go: (u) => opened.push(u), href: () => 'https://accounts.google.com/v3/signin/rejected?TL=abc', text: () => '' });
  onTestFinished(() => h.destroy());
  expect(opened).toEqual(['https://youtu.be/abcDEF12345']);
});

test('an address that is not YouTube gets no button', () => {
  const h = signInHelp({ store: memoryStore(), go: () => {}, href: () => gate('http://evil.example/youtube.com'), text: () => '' });
  onTestFinished(() => h.destroy());
  expect(skipButton()).toBeNull();
});
