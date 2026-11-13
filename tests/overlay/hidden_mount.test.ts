// An overlay first mounted while the page is hidden (the app was in the
// background when the page loaded) looks up the page's job when the page comes
// back, and shows its subtitles (R6, R11).

import { expect, onTestFinished, test } from 'vitest';
import { mountOverlay } from '../../src/overlay/index';

const CUE = { seq: 1, start_ms: 0, end_ms: 60_000, text_ko: '어텐션은 질의와 키의 내적으로 가중치를 구합니다.' };

test('test_an_overlay_mounted_while_hidden_loads_the_existing_job_when_the_page_is_shown', async () => {
  let state: DocumentVisibilityState = 'hidden';
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => state });
  onTestFinished(() => void delete (document as any).visibilityState);

  document.body.innerHTML = '';
  const video = document.createElement('video');
  video.style.cssText = 'display:block;width:640px;height:360px';
  Object.defineProperty(video, 'currentTime', { configurable: true, get: () => 5 });
  Object.defineProperty(video, 'duration', { configurable: true, get: () => 120 });
  Object.defineProperty(video, 'paused', { configurable: true, get: () => true });
  document.body.append(video);

  const calls: string[] = [];
  const o = mountOverlay({
    video,
    pageUrl: 'https://lectures.example.org/attention',
    title: 'Attention',
    mediaUrl: null,
    request: async (method, path) => {
      calls.push(`${method} ${path}`);
      if (method === 'GET' && path === '/jobs') return { status: 200, body: { stage: 'ready', video_key: 'vk-1', percent: 100 } };
      if (path.startsWith('/videos/')) return { status: 200, body: { video_key: 'vk-1', duration_ms: 120_000, cues: [CUE] } };
      return { status: 404, body: {} };
    },
  });
  onTestFinished(() => o.destroy());
  const sub = document.querySelector('[data-vto="overlay"]')!.shadowRoot!.querySelector('[data-vto="subtitle"]') as HTMLElement;

  await new Promise((r) => setTimeout(r, 300));
  expect(calls, 'nothing is asked while the page is hidden').toEqual([]);

  state = 'visible';
  document.dispatchEvent(new Event('visibilitychange'));
  await expect.poll(() => sub.textContent, { timeout: 5_000 }).toBe(CUE.text_ko);
  expect(calls).toEqual(['GET /jobs', 'GET /videos/vk-1/subtitles']);

  // A later return asks nothing again: the ready job's subtitles are loaded.
  document.dispatchEvent(new Event('visibilitychange'));
  await new Promise((r) => setTimeout(r, 300));
  expect(calls).toHaveLength(2);
});
