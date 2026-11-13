// An ad must never become the lecture (R18, R22): a blob player that loaded
// an ad's playlist first sends the lecture's, and 영상감지 pressed during a
// YouTube ad waits for the ad to end before it sends the player's length.

import { expect, onTestFinished, test } from 'vitest';
import { mountOverlay, videoMediaUrl } from '../../src/overlay/index';

const FIX = '/tests/overlay/fixtures';

/** A video whose length the test sets, playing from a MediaSource (blob:) when `blob`. */
function video(seconds: number, blob = true): { v: HTMLVideoElement; set(s: number): void } {
  const v = document.createElement('video');
  if (blob) v.src = URL.createObjectURL(new MediaSource());
  let duration = seconds;
  Object.defineProperty(v, 'duration', { configurable: true, get: () => duration });
  document.body.append(v);
  onTestFinished(() => v.remove());
  return { v, set: (s) => (duration = s) };
}

async function load(path: string) {
  await fetch(path);
  await expect.poll(() => performance.getEntriesByType('resource').some((e) => e.name.endsWith(path))).toBe(true);
}

test('test_blob_player_sends_the_playlist_as_long_as_the_video', async () => {
  const { v } = video(3600);
  const since = performance.now();
  await load(`${FIX}/ad.m3u8`);
  await load(`${FIX}/lecture.m3u8`);
  expect(await videoMediaUrl(v, since)).toBe(`${location.origin}${FIX}/lecture.m3u8`);

  // A master playlist is measured by its variant.
  const later = performance.now();
  await load(`${FIX}/ad.m3u8?again`);
  await load(`${FIX}/master.m3u8`);
  expect(await videoMediaUrl(v, later)).toBe(`${location.origin}${FIX}/master.m3u8`);

  // Only the ad's has been loaded while the lecture plays: nothing is sent.
  const { v: w } = video(3600);
  const last = performance.now();
  await load(`${FIX}/ad.m3u8?only`);
  expect(await videoMediaUrl(w, last)).toBeNull();
});

test('test_detect_during_a_youtube_ad_waits_for_the_lecture', async () => {
  const player = document.createElement('div');
  player.className = 'html5-video-player ad-showing';
  document.body.append(player);
  onTestFinished(() => player.remove());
  const { v, set } = video(30, false); // the ad's 30 s
  player.append(v);
  const posts: any[] = [];
  const request = async (method: string, path: string, body?: unknown) => {
    if (method === 'POST' && path === '/jobs') {
      posts.push(body);
      return { status: 200, body: { stage: 'downloading', video_key: 'vk' } };
    }
    return { status: 404, body: { error: 'no_job' } };
  };
  const o = mountOverlay({ video: v, pageUrl: 'https://www.youtube.com/watch?v=abc', title: 't', mediaUrl: null, request });
  onTestFinished(() => o.destroy());

  const host = document.querySelector('[data-vto="overlay"]')!;
  (host.shadowRoot!.querySelector('[data-vto="detect"]') as HTMLButtonElement).click();
  await new Promise((r) => setTimeout(r, 600));
  expect(posts).toEqual([]);
  expect(host.shadowRoot!.querySelector('[data-vto="status"]')!.textContent).toContain('광고');

  // The ad ends; the lecture's hour loads.
  player.classList.remove('ad-showing');
  set(3600);
  await expect.poll(() => posts.length).toBe(1);
  expect(posts[0].duration_ms).toBe(3_600_000);
});
