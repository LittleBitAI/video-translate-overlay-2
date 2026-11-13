// A lecture page that embeds its player in an iframe gets the overlay on that
// player's video, and the job is the page's (R12, R17). The test's own frame
// plays the top frame; `fixtures/frame.html` is the embedded player.

import { expect, onTestFinished, test } from 'vitest';
import { startPage } from '../../src/overlay/page';

type Call = { method: string; path: string; body?: any };

function deepAll(selector: string, root: Document | ShadowRoot): Element[] {
  const found = [...root.querySelectorAll(selector)];
  for (const el of root.querySelectorAll('*')) if (el.shadowRoot) found.push(...deepAll(selector, el.shadowRoot));
  return found;
}

test('test_a_video_in_an_iframe_gets_the_overlay', async () => {
  document.body.innerHTML = '';
  document.body.style.margin = '0';
  // A small video on the page itself (a teaser) and the lecture's player in a frame.
  const teaser = document.createElement('video');
  teaser.style.cssText = 'display:block;width:160px;height:90px';
  const frame = document.createElement('iframe');
  frame.style.cssText = 'display:block;width:640px;height:360px;border:0';
  frame.src = '/tests/overlay/fixtures/frame.html';
  document.body.append(teaser, frame);
  onTestFinished(() => {
    teaser.remove();
    frame.remove();
  });

  const calls: Call[] = [];
  const page = startPage({
    top: null,
    request: async (method, path, body) => {
      calls.push({ method, path, body });
      if (method === 'POST') return { status: 200, body: { job_id: 'job-1', video_key: 'vk-1', stage: 'downloading', reused: false } };
      return { status: 404, body: { error: 'no_job' } };
    },
  });
  onTestFinished(() => page.stop());

  const inFrame = () => (frame.contentDocument ? deepAll('[data-vto="detect"]', frame.contentDocument) : []);
  await expect.poll(() => inFrame().length, { timeout: 10_000 }).toBe(1);
  expect(deepAll('[data-vto="detect"]', document), 'the teaser has no overlay of its own').toHaveLength(0);

  (inFrame()[0] as HTMLButtonElement).click();
  await expect.poll(() => calls.some((c) => c.method === 'POST'), { timeout: 5_000 }).toBe(true);
  const post = calls.find((c) => c.method === 'POST')!;
  expect(post.path).toBe('/jobs');
  // The page's address and title, the frame's media.
  expect(post.body.page_url).toBe(location.href);
  expect(post.body.title).toBe(document.title.trim() || 'Embedded lecture player');
  expect(post.body.media_url).toBe(`${location.origin}/vto-none/lecture.mp4`);
  for (const c of calls.filter((c) => c.path === '/jobs')) expect(c.body.page_url).toBe(location.href);

  // The player goes: the page's own video gets the overlay.
  frame.remove();
  await expect.poll(() => deepAll('[data-vto="detect"]', document).length, { timeout: 10_000 }).toBe(1);
});
