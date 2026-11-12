// The media URL is read when 영상감지 is pressed, not when the overlay mounts:
// many sites show the player first and set its source later.

import { expect, onTestFinished, test } from 'vitest';
import { mountOverlay, videoMediaUrl } from '../../src/overlay/index';

const PAGE = 'https://lectures.example.org/talk/42';

function video(): HTMLVideoElement {
  const v = document.createElement('video');
  v.style.width = '640px';
  v.style.height = '360px';
  document.body.append(v);
  onTestFinished(() => v.remove());
  return v;
}

test('test_media_set_after_mount_is_sent', async () => {
  const v = video();
  const posts: any[] = [];
  const request = async (method: string, path: string, body?: unknown) => {
    if (method === 'POST' && path === '/jobs') {
      posts.push(body);
      return { status: 200, body: { stage: 'downloading', percent: 0, video_key: 'vk' } };
    }
    return { status: 404, body: { error: 'no_job' } };
  };
  const o = mountOverlay({ video: v, pageUrl: PAGE, title: 't', mediaUrl: () => videoMediaUrl(v), request });
  onTestFinished(() => o.destroy());

  // The player gets its source only after the overlay is up.
  v.src = 'https://cdn.example.org/talk42.mp4';
  const host = document.querySelector('[data-vto="overlay"]')!;
  (host.shadowRoot!.querySelector('[data-vto="detect"]') as HTMLButtonElement).click();

  await expect.poll(() => posts.length).toBe(1);
  expect(posts[0].media_url).toBe('https://cdn.example.org/talk42.mp4');
});

test('test_blob_and_missing_sources_send_no_media', async () => {
  const v = video();
  expect(await videoMediaUrl(v)).toBeNull();
  v.src = 'blob:https://www.youtube.com/0b1c';
  expect(await videoMediaUrl(v)).toBeNull();
  const w = video();
  const s = document.createElement('source');
  s.src = 'https://cdn.example.org/a.webm';
  w.append(s);
  expect(await videoMediaUrl(w)).toBe('https://cdn.example.org/a.webm');
});
