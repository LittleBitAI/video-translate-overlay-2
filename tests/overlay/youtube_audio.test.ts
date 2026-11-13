import { expect, onTestFinished, test, vi } from 'vitest';
import { mountOverlay, videoMediaUrl } from '../../src/overlay/index';

const PAGE = 'https://m.youtube.com/watch?v=jNQXAC9IVRw';
const AUDIO = 'https://rr1.googlevideo.com/videoplayback?source=youtube&mime=audio%2Fmp4&dur=19.021&sig=session-signature';

function fixture() {
  const video = document.createElement('video');
  video.src = URL.createObjectURL(new MediaSource());
  Object.defineProperty(video, 'duration', { value: 19.021 });
  document.body.append(video);
  onTestFinished(() => { URL.revokeObjectURL(video.src); video.remove(); });
  const player = {
    playabilityStatus: { status: 'OK' }, videoDetails: { videoId: 'jNQXAC9IVRw' },
    streamingData: { adaptiveFormats: [
      { mimeType: 'audio/webm; codecs="opus"', bitrate: 160000, url: AUDIO.replace('mp4', 'webm') },
      { mimeType: 'audio/mp4; codecs="mp4a.40.2"', bitrate: 130000, url: AUDIO },
    ] },
  };
  const fetcher = vi.spyOn(window, 'fetch').mockImplementation(async () => new Response(JSON.stringify(player), { status: 200 }));
  onTestFinished(() => fetcher.mockRestore());
  return { video, player, fetcher };
}

test('YouTube audio is requested in the page session and its signed AAC URL reaches the job', async () => {
  const { video, fetcher } = fixture();
  const posts: any[] = [];
  const overlay = mountOverlay({ video, pageUrl: PAGE, title: 'zoo', mediaUrl: () => videoMediaUrl(video, 0, PAGE),
    request: async (method, _path, body) => {
      if (method === 'POST') posts.push(body);
      return { status: 404, body: {} };
    } });
  onTestFinished(() => overlay.destroy());
  (document.querySelector('[data-vto=overlay]')!.shadowRoot!.querySelector('[data-vto=detect]') as HTMLButtonElement).click();
  await expect.poll(() => posts.length).toBe(1);
  expect(posts[0].media_url).toBe(AUDIO);
  const [endpoint, options] = fetcher.mock.calls[0];
  expect(endpoint).toBe('/youtubei/v1/player?prettyPrint=false');
  expect(options?.credentials).toBe('same-origin');
  expect(JSON.parse(options?.body as string)).toMatchObject({ videoId: 'jNQXAC9IVRw', context: { client: { clientName: 'VISIONOS' } } });
});

test('YouTube audio never uses another video, an ad, a nondefault track or a live stream', async () => {
  const { video, player } = fixture();
  player.videoDetails.videoId = 'another0001';
  expect(await videoMediaUrl(video, 0, PAGE)).toBeNull();
  player.videoDetails.videoId = 'jNQXAC9IVRw';
  for (const flag of ['isLive', 'isUpcoming']) {
    (player.videoDetails as any)[flag] = true;
    expect(await videoMediaUrl(video, 0, PAGE)).toBeNull();
    delete (player.videoDetails as any)[flag];
  }
  for (const format of player.streamingData.adaptiveFormats) format.url = format.url.replace('dur=19.021', 'dur=30');
  expect(await videoMediaUrl(video, 0, PAGE)).toBeNull();
  for (const format of player.streamingData.adaptiveFormats) {
    format.url = AUDIO;
    (format as any).audioTrack = { audioIsDefault: false };
  }
  expect(await videoMediaUrl(video, 0, PAGE)).toBeNull();
});

test('YouTube player failures and untrusted stream URLs fall back without leaving the button waiting', async () => {
  const { video, player, fetcher } = fixture();
  for (const format of player.streamingData.adaptiveFormats) format.url = AUDIO.replace('rr1.googlevideo.com', 'googlevideo.com.evil.test');
  expect(await videoMediaUrl(video, 0, PAGE)).toBeNull();
  fetcher.mockRejectedValueOnce(new TypeError('offline'));
  expect(await videoMediaUrl(video, 0, PAGE)).toBeNull();
  fetcher.mockResolvedValueOnce(new Response('denied', { status: 403 }));
  expect(await videoMediaUrl(video, 0, PAGE)).toBeNull();
});
