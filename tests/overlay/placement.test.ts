// The overlay lies over the video, not at the bottom of the screen, and goes
// into the player when the player is in full screen (R4, R8, R12).

import { expect, onTestFinished, test } from 'vitest';
import { page } from 'vitest/browser';
import { mountOverlay } from '../../src/overlay/index';

const CUE = { seq: 1, start_ms: 0, end_ms: 60_000, text_ko: '어텐션은 질의와 키의 내적으로 가중치를 구합니다.' };

function setup(width: string, height: string) {
  document.body.innerHTML = '';
  document.body.style.margin = '0';
  const player = document.createElement('div');
  player.style.cssText = `position:relative;margin-top:40px;width:${width};height:${height};background:#000`;
  const video = document.createElement('video');
  video.style.cssText = 'display:block;width:100%;height:100%';
  Object.defineProperty(video, 'currentTime', { configurable: true, get: () => 5 });
  Object.defineProperty(video, 'duration', { configurable: true, get: () => 120 });
  Object.defineProperty(video, 'paused', { configurable: true, get: () => true });
  player.append(video);
  const below = document.createElement('p');
  below.textContent = 'Lecture notes.';
  document.body.append(player, below);
  const o = mountOverlay({
    video,
    pageUrl: 'https://lectures.example.org/attention',
    title: 'Attention',
    mediaUrl: null,
    request: async (method, path) => {
      if (method === 'GET' && path === '/jobs') return { status: 200, body: { stage: 'ready', video_key: 'vk-1', percent: 100 } };
      if (path.startsWith('/videos/')) return { status: 200, body: { video_key: 'vk-1', duration_ms: 120_000, cues: [CUE] } };
      return { status: 404, body: {} };
    },
  });
  onTestFinished(() => o.destroy());
  const host = document.querySelector('[data-vto="overlay"]') as HTMLElement;
  const part = (sel: string) => host.shadowRoot!.querySelector(sel) as HTMLElement;
  return { player, video, host, button: part('[data-vto="detect"]'), sub: part('[data-vto="subtitle"]'), root: part('.root') };
}

const inside = (inner: DOMRect, outer: DOMRect) =>
  inner.left >= outer.left - 1 && inner.top >= outer.top - 1 && inner.right <= outer.right + 1 && inner.bottom <= outer.bottom + 1;

test('test_overlay_lies_over_the_video_and_follows_it_into_full_screen', async () => {
  await page.viewport(384, 854);
  const o = setup('100%', '216px');
  await expect.poll(() => o.sub.textContent, { timeout: 5_000 }).toBe(CUE.text_ko);
  const v = o.video.getBoundingClientRect();
  const b = o.button.getBoundingClientRect();
  const s = o.sub.getBoundingClientRect();
  expect(inside(b, v), `button ${JSON.stringify(b)} not over the video ${JSON.stringify(v)}`).toBe(true);
  expect(inside(s, v), `subtitle ${JSON.stringify(s)} not over the video ${JSON.stringify(v)}`).toBe(true);
  expect(s.top > v.top + v.height / 2, 'the subtitle sits in the lower half of the video').toBe(true);

  // The player goes full screen (stood in for: a test cannot ask for real full screen without a gesture).
  Object.defineProperty(document, 'fullscreenElement', { configurable: true, get: () => o.player });
  onTestFinished(() => void delete (document as any).fullscreenElement);
  document.dispatchEvent(new Event('fullscreenchange'));
  expect(o.host.parentElement).toBe(o.player);
  delete (document as any).fullscreenElement;
  document.dispatchEvent(new Event('fullscreenchange'));
  expect(o.host.parentElement).toBe(document.body);

  // The page scrolls: the overlay goes with the video.
  window.scrollTo(0, 0);
  const before = o.root.getBoundingClientRect().top;
  document.body.style.minHeight = '3000px';
  window.scrollTo(0, 30);
  await expect.poll(() => o.root.getBoundingClientRect().top, { timeout: 2_000 }).toBe(Math.max(0, before - 30));
  window.scrollTo(0, 0);
});

test('test_a_video_too_small_to_hold_it_puts_the_overlay_at_the_bottom', async () => {
  await page.viewport(1280, 720);
  const o = setup('120px', '68px');
  await expect.poll(() => o.sub.textContent, { timeout: 5_000 }).toBe(CUE.text_ko);
  const b = o.button.getBoundingClientRect();
  expect(b.bottom).toBeGreaterThan(innerHeight - 80);
  expect(b.right <= innerWidth && b.left >= 0).toBe(true);
});
