// A video played through MediaSource (hls.js and the like) has a blob: source;
// the playlist the page fetched for it is sent instead (R17).

import { expect, onTestFinished, test } from 'vitest';
import { videoMediaUrl } from '../../src/overlay/index';

function blobVideo(): HTMLVideoElement {
  const v = document.createElement('video');
  v.src = URL.createObjectURL(new MediaSource());
  document.body.append(v);
  onTestFinished(() => v.remove());
  return v;
}

/** What a player's script does: fetch the playlist (whether it answers does not matter here). */
async function load(path: string) {
  await fetch(path).catch(() => undefined);
  await expect.poll(() => performance.getEntriesByType('resource').some((e) => e.name.includes(path))).toBe(true);
}

test('test_blob_player_sends_the_playlist_it_fetched', async () => {
  const v = blobVideo();
  expect(videoMediaUrl(v, performance.now())).toBeNull();
  await load('/vto-mse/other.js');
  const since = performance.now();
  await load('/vto-mse/master.m3u8?token=1');
  await load('/vto-mse/v720/index.m3u8');
  // The first playlist since the page changed: the master, which the backend probes for its variants.
  expect(videoMediaUrl(v, since)).toBe(`${location.origin}/vto-mse/master.m3u8?token=1`);
  // Playlists from before the page changed belong to another video.
  expect(videoMediaUrl(v, performance.now())).toBeNull();
});

test('test_blob_dash_player_sends_its_manifest', async () => {
  const v = blobVideo();
  const since = performance.now();
  await load('/vto-mse/talk.mpd');
  expect(videoMediaUrl(v, since)).toBe(`${location.origin}/vto-mse/talk.mpd`);
});
