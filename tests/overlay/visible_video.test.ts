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

// A frame's video counts only by the part of it in view through every frame
// above it. In each row a 160×90 lecture is in view on the page, and the
// larger player (300×150, wholly in view inside its own frame) is somewhere in a frame.
const FRAMED: { layout: string; src: string; css: string; nested: boolean; want: 'page' | 'player' }[] = [
  { layout: 'a direct frame below the viewport', src: 'frame.html', css: 'top:1000px;width:1280px;height:720px', nested: false, want: 'page' },
  { layout: 'a nested player in a frame below the viewport', src: 'outer.html', css: 'top:1000px;width:800px;height:500px', nested: true, want: 'page' },
  { layout: 'a nested player out of view inside a frame in view', src: 'outer.html?at=600', css: 'top:300px;width:800px;height:400px', nested: true, want: 'page' },
  { layout: 'a nested player in view', src: 'outer.html', css: 'top:200px;width:800px;height:500px', nested: true, want: 'player' },
];

test.each(FRAMED)('test_a_frame_video_counts_only_by_its_part_in_view_through_every_frame_above_it: $layout', async ({ src, css, nested, want }) => {
  await page.viewport(1280, 800);
  lay([{ name: 'lecture', css: 'left:0;top:0;width:160px;height:90px' }]);
  const frame = document.createElement('iframe');
  frame.style.cssText = `position:absolute;left:0;border:0;${css}`;
  frame.src = `/tests/overlay/fixtures/${src}`;
  document.body.append(frame);
  const p = startPage({ top: null, request: async () => ({ status: 404, body: { error: 'no_job' } }) });
  onTestFinished(() => p.stop());

  const player = (): Document | null => {
    const d = frame.contentDocument;
    return (nested ? (d?.getElementById('player') as HTMLIFrameElement | null)?.contentDocument : d) ?? null;
  };
  const overlayIn = (d: Document | null) => !!d?.querySelector('[data-vto="overlay"]');
  await expect.poll(() => player()?.readyState === 'complete' && !!player()?.querySelector('video'), { timeout: 10_000 }).toBe(true);

  if (want === 'player') {
    await expect.poll(() => overlayIn(player()), { timeout: 10_000 }).toBe(true);
    expect(overlayIn(document), 'the lecture gives the overlay up to the larger player in view').toBe(false);
  } else {
    // Each frame tells its own frames how much of them is in view once a second; give the chain time to settle.
    await new Promise((r) => setTimeout(r, 5_000));
    expect(overlayIn(document), 'the lecture in view keeps the overlay').toBe(true);
    expect(overlayIn(player()), 'the player out of view gets none').toBe(false);
  }
});
