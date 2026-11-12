// The overlay goes on the video the viewer sees: the one with the most of it on
// screen, not the one with the largest box (R12). A video above or below the
// viewport, hidden by CSS, or in a frame that is off the page loses to a smaller
// one in view; with none in view, the largest shown one keeps it.

import { expect, onTestFinished, test } from 'vitest';
import { page } from 'vitest/browser';
import { mainVideo, startPage } from '../../src/overlay/page';

type Box = { name: string; css: string };

function lay(boxes: Box[]): Record<string, HTMLVideoElement> {
  document.body.innerHTML = '';
  document.body.style.margin = '0';
  const out: Record<string, HTMLVideoElement> = {};
  for (const b of boxes) {
    const v = document.createElement('video');
    v.style.cssText = `position:absolute;display:block;${b.css}`;
    v.dataset.name = b.name;
    document.body.append(v);
    out[b.name] = v;
  }
  onTestFinished(() => (document.body.innerHTML = ''));
  return out;
}

// The viewport is 1280×800 in every row.
const LAYOUTS: { layout: string; boxes: Box[]; want: string }[] = [
  {
    layout: 'a larger video above the viewport',
    boxes: [
      { name: 'outside', css: 'left:0;top:-760px;width:1280px;height:720px' },
      { name: 'inside', css: 'left:0;top:100px;width:640px;height:360px' },
    ],
    want: 'inside',
  },
  {
    layout: 'a larger video below the viewport',
    boxes: [
      { name: 'inside', css: 'left:0;top:0;width:640px;height:360px' },
      { name: 'outside', css: 'left:0;top:900px;width:1280px;height:720px' },
    ],
    want: 'inside',
  },
  {
    layout: 'a larger video mostly off the side of the viewport',
    boxes: [
      { name: 'outside', css: 'left:1200px;top:0;width:1280px;height:720px' },
      { name: 'inside', css: 'left:0;top:400px;width:640px;height:360px' },
    ],
    want: 'inside',
  },
  {
    layout: 'a larger video hidden with visibility',
    boxes: [
      { name: 'outside', css: 'left:0;top:0;width:1280px;height:720px;visibility:hidden' },
      { name: 'inside', css: 'left:0;top:0;width:640px;height:360px' },
    ],
    want: 'inside',
  },
  {
    layout: 'a larger video at opacity 0',
    boxes: [
      { name: 'outside', css: 'left:0;top:0;width:1280px;height:720px;opacity:0' },
      { name: 'inside', css: 'left:0;top:0;width:640px;height:360px' },
    ],
    want: 'inside',
  },
  {
    layout: 'two videos in view',
    boxes: [
      { name: 'small', css: 'left:0;top:0;width:320px;height:180px' },
      { name: 'large', css: 'left:0;top:200px;width:960px;height:540px' },
    ],
    want: 'large',
  },
  {
    layout: 'no video in view (the page is scrolled past its player)',
    boxes: [
      { name: 'small', css: 'left:0;top:-400px;width:320px;height:180px' },
      { name: 'large', css: 'left:0;top:-1000px;width:960px;height:540px' },
    ],
    want: 'large',
  },
];

test('test_the_main_video_is_the_one_most_on_screen_across_layouts', async () => {
  await page.viewport(1280, 800);
  const got = LAYOUTS.map(({ layout, boxes }) => {
    lay(boxes);
    return { layout, name: mainVideo()?.dataset.name };
  });
  expect(got).toEqual(LAYOUTS.map(({ layout, want }) => ({ layout, name: want })));
});

test('test_detect_on_a_page_with_an_offscreen_larger_video_sends_the_visible_lecture', async () => {
  await page.viewport(1280, 800);
  const v = lay([
    { name: 'outside', css: 'left:0;top:-760px;width:1280px;height:720px' },
    { name: 'inside', css: 'left:0;top:100px;width:640px;height:360px' },
  ]);
  const media = (el: HTMLVideoElement, src: string, s: number) => {
    Object.defineProperty(el, 'currentSrc', { configurable: true, get: () => src });
    Object.defineProperty(el, 'duration', { configurable: true, get: () => s });
  };
  media(v.outside, 'https://media.example.org/outside/target.mp4', 30);
  media(v.inside, 'https://media.example.org/lecture.mp4', 3_600);

  const posts: any[] = [];
  const p = startPage({
    top: null,
    request: async (method, _path, body) => {
      if (method === 'POST') {
        posts.push(body);
        return { status: 200, body: { job_id: 'job-1', video_key: 'vk-1', stage: 'downloading', reused: false } };
      }
      return { status: 404, body: { error: 'no_job' } };
    },
  });
  onTestFinished(() => p.stop());

  const host = document.querySelector('[data-vto="overlay"]') as HTMLElement;
  const button = host.shadowRoot!.querySelector('[data-vto="detect"]') as HTMLButtonElement;
  // The controls lie over the visible lecture, not across the bottom of the screen.
  const b = button.getBoundingClientRect();
  const r = v.inside.getBoundingClientRect();
  expect(b.top >= r.top - 1 && b.bottom <= r.bottom + 1, `button ${JSON.stringify(b)} over ${JSON.stringify(r)}`).toBe(true);

  button.click();
  await expect.poll(() => posts.length, { timeout: 5_000 }).toBe(1);
  expect(posts[0].media_url).toBe('https://media.example.org/lecture.mp4');
  expect(posts[0].duration_ms).toBe(3_600_000);
});

test('test_a_frame_off_the_page_does_not_take_the_overlay_from_a_video_in_view', async () => {
  await page.viewport(1280, 800);
  lay([{ name: 'inside', css: 'left:0;top:0;width:320px;height:180px' }]);
  // The embedded player is larger, and wholly in view inside its own frame, but the frame is below the viewport.
  const frame = document.createElement('iframe');
  frame.style.cssText = 'position:absolute;left:0;top:1000px;width:1280px;height:720px;border:0';
  frame.src = '/tests/overlay/fixtures/frame.html';
  document.body.append(frame);
  const p = startPage({ top: null, request: async () => ({ status: 404, body: { error: 'no_job' } }) });
  onTestFinished(() => p.stop());

  await expect.poll(() => frame.contentDocument?.querySelector('video') !== null && frame.contentDocument?.readyState, { timeout: 10_000 }).toBe('complete');
  // Give the frame time to report its video (it does so every second).
  await new Promise((r) => setTimeout(r, 2_500));
  const ownHost = document.querySelector('[data-vto="overlay"]');
  expect(ownHost, 'the video in view keeps the overlay').not.toBeNull();
  expect(frame.contentDocument!.querySelector('[data-vto="overlay"]'), 'the frame off the page gets none').toBeNull();
});
