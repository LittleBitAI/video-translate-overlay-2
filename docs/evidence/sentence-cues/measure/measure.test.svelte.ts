// The rendered-line measurement behind the README's tables (done item 10). Not part of
// the repository gate: its input is the person's subtitles, which stay out of Git.
//
//   npx vitest run -c docs/evidence/sentence-cues/measure/measure.config.ts
//
// It reads artifacts/sentence-cues/ko.json, {old|new: {video_key: [[seq, text_ko]]}}: every
// Korean cue of the eight videos' old and new saved versions, exported from the account
// library (receipts.md gives its SHA-256). Each cue is rendered in the page overlay and in
// the Watch screen at phone, windowed PC and fullscreen PC sizes, and its lines are counted
// with the line tests' own `lineWidths`. The test asserts that the count of cues on each
// number of lines, and the list of cues over two lines, equal expected.json for every
// surface, size, version and video. The README's tables are sums of expected.json.
import { mount, unmount } from 'svelte';
import { expect, test, vi } from 'vitest';
import { commands, page } from 'vitest/browser';
import Overlay from '../../../../src/ui/Overlay.svelte';
import '../../../../src/ui/app.css';
import { mountOverlay } from '../../../../src/overlay/index';
import { lineWidths } from '../../../../tests/subtitle-lines';
import { api, calls } from '../../../../tests/ui/fake';
import expected from './expected.json';
import data from '../../../../artifacts/sentence-cues/ko.json';

type Data = Record<string, Record<string, [number, string][]>>;
type Counts = Record<string, Record<string, { lines: Record<string, number>; over: [number, number][] }>>;
const SIZES = { phone: [384, 854], windowed: [1280, 800], fullscreen: [1920, 1080] } as const;
// The page overlay's player and the Watch screen's player at each size (as the line tests use).
const OVERLAY_PLAYER = { phone: ['100%', '216px'], windowed: ['100%', '720px'], fullscreen: ['100%', '1080px'] } as const;
const WATCH_PLAYER = { phone: [352, 198], windowed: [1248, 702], fullscreen: [1920, 1080] } as const;

/** For each version and video: how many cues render on each number of lines, and [seq, lines] of those over two. */
function count(measure: (t: string) => number): Counts {
  const out: Counts = {};
  for (const [label, videos] of Object.entries(data as Data).sort()) {
    for (const [key, cues] of Object.entries(videos).sort()) {
      const lines: Record<string, number> = {};
      const over: [number, number][] = [];
      for (const [seq, t] of cues) {
        const n = measure(t);
        lines[n] = (lines[n] ?? 0) + 1;
        if (n > 2) over.push([seq, n]);
      }
      (out[label] ??= {})[key] = { lines, over };
    }
  }
  return out;
}

const measured: Record<string, Record<string, Counts>> = { overlay: {}, watch: {} };

test('page overlay', async () => {
  for (const [size, [w, h]] of Object.entries(SIZES)) {
    await page.viewport(w, h);
    document.body.innerHTML = '';
    document.body.style.margin = '0';
    const player = document.createElement('div');
    const [pw, ph] = OVERLAY_PLAYER[size as keyof typeof SIZES];
    player.style.cssText = `position:relative;margin-top:40px;width:${pw};height:${ph};background:#000`;
    const video = document.createElement('video');
    video.style.cssText = 'display:block;width:100%;height:100%';
    Object.defineProperty(video, 'currentTime', { configurable: true, get: () => 5 });
    Object.defineProperty(video, 'duration', { configurable: true, get: () => 120 });
    Object.defineProperty(video, 'paused', { configurable: true, get: () => true });
    player.append(video);
    document.body.append(player);
    const o = mountOverlay({ video, pageUrl: 'https://lectures.example.org/x', title: 'x', mediaUrl: null,
      request: async (method, path) => {
        if (method === 'GET' && path === '/jobs') return { status: 200, body: { stage: 'ready', video_key: 'vk', percent: 100 } };
        if (path.startsWith('/videos/')) return { status: 200, body: { video_key: 'vk', duration_ms: 120_000, cues: [{ seq: 1, start_ms: 0, end_ms: 60_000, text_ko: '시작' }] } };
        return { status: 404, body: {} };
      } });
    const sub = document.querySelector('[data-vto="overlay"]')!.shadowRoot!.querySelector('[data-vto="subtitle"]') as HTMLElement;
    await expect.poll(() => sub.textContent, { timeout: 5_000 }).toBe('시작');
    measured.overlay[size] = count((t) => { sub.textContent = t; return lineWidths(sub).length; });
    o.destroy();
  }
  await commands.writeFile('artifacts/sentence-cues/measured-overlay.json', JSON.stringify(measured.overlay));
  expect(measured.overlay).toEqual(expected.overlay);
}, 600_000);

test('watch screen', async () => {
  api((c) => c.path === '/jobs' ? [200, { stage: 'ready', percent: 100, cost_krw: 3, failure_reason: null, video_key: 'vk' }] : [200, { duration_ms: 600000, cues: [{ seq: 0, start_ms: 1000, end_ms: 4000, text_ko: '시작' }] }]);
  document.body.innerHTML = '';
  const target = document.body.appendChild(document.createElement('div'));
  const video = target.appendChild(document.createElement('video'));
  video.style.cssText = 'width:100%;height:100%';
  const shown = mount(Overlay, { target, props: { pageUrl: 'https://talks.example.org/7', video, timeMs: 1500, durationMs: 600000,
    initial: { stage: 'ready', percent: 100, cost_krw: 3, failure_reason: null, video_key: 'vk' } } });
  for (const [size, [w, h]] of Object.entries(SIZES)) {
    await page.viewport(w, h);
    const [pw, ph] = WATCH_PLAYER[size as keyof typeof SIZES];
    target.style.cssText = `position:fixed;inset:0 auto auto 0;width:${pw}px;height:${ph}px`;
    let sub: HTMLElement | null = null;
    await vi.waitFor(() => { sub = target.querySelector<HTMLElement>('[data-feature="watch-subtitles"]'); expect(sub?.textContent?.trim(), JSON.stringify(calls)).toBe('시작'); }, { timeout: 15_000 });
    await new Promise((r) => setTimeout(r, 300));
    measured.watch[size] = count((t) => { sub!.textContent = t; return lineWidths(sub!).length; });
    sub!.textContent = '시작';
  }
  unmount(shown);
  await commands.writeFile('artifacts/sentence-cues/measured-watch.json', JSON.stringify(measured.watch));
  expect(measured.watch).toEqual(expected.watch);
  // The new versions: no cue over two lines in the page overlay at any size, nor in the Watch
  // screen at PC sizes; the Watch screen on a phone has the twelve exceptions the README lists.
  const over = (s: 'overlay' | 'watch', size: string) => Object.values(measured[s][size].new).reduce((n, v) => n + v.over.length, 0);
  expect(['phone', 'windowed', 'fullscreen'].map((z) => [over('overlay', z), over('watch', z)])).toEqual([[0, 12], [0, 0], [0, 0]]);
}, 600_000);
