// Execution evidence for the 요약 button (P19) on a running stand-in server
// (`PORT=… APP_DATA_DIR=<empty dir> node scripts/live.mjs --stand-ins`).
// It seeds e2e/seed.json, then records isolated P19 requests and browser
// actions at 1280x720 and 384x854, each as expected vs observed, and the
// server's build head. It ties the server, every page Chromium loaded and the
// checkout it runs in to one commit, and leaves and reopens the video while a
// stand-in summary call is out (the stand-in takes 3 s to summarize).
// Usage: node scripts/verification/video-summary.mjs <base-url> <out-dir>
// Writes <out-dir>/report.json and <out-dir>/<size>-<state>.png; exits 1 if any check fails.
import { chromium } from 'playwright';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const [base, out] = process.argv.slice(2);
if (!base || !out) throw new Error('usage: video-summary.mjs <base-url> <out-dir>');
mkdirSync(out, { recursive: true });
const checks = [];
const check = (where, what, expected, observed, pass = JSON.stringify(expected) === JSON.stringify(observed)) => {
  checks.push({ where, what, expected, observed, pass });
  console.log(pass ? 'PASS' : 'FAIL', where, what, JSON.stringify(observed));
};
const call = async (method, path, body, headers = {}) => {
  const r = await fetch(base + path, { method, headers: { 'Content-Type': 'application/json', ...headers }, body: body ? JSON.stringify(body) : undefined });
  return { status: r.status, body: await r.json().catch(() => null) };
};

const build = (await call('GET', '/build')).body;
// The checkout this script runs in, and what it has not committed (the wiki's files are not built).
const git = (...a) => execFileSync('git', a, { encoding: 'utf8' });
const checkout = { head: git('rev-parse', 'HEAD').trim(), uncommitted: git('status', '--porcelain').split('\n').filter((l) => l && !l.slice(3).startsWith('.wiki/')) };
check('build', 'the server was built at the checkout, which has nothing uncommitted', [checkout.head, []], [build?.head, checkout.uncommitted]);
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
for (const step of JSON.parse(readFileSync('e2e/seed.json', 'utf8'))) await call(step.method, step.path, step.body);
const PAGE = 'https://www.youtube.com/watch?v=kCc8FmEb1nY';
const job = async () => (await call('GET', `/jobs?page_url=${encodeURIComponent(PAGE)}`)).body;
for (let i = 0; (await job())?.stage !== 'ready'; i++) {
  if (i > 600) throw new Error('the seeded job did not become ready');
  await new Promise((r) => setTimeout(r, 200));
}
const key = (await job()).video_key;
const path = `/videos/${encodeURIComponent(key)}/summary`;
const round = (n) => Math.round(n * 1e9) / 1e9;

// ---- API: isolated P19 requests
const translated = await job();
let r = await call('GET', path);
check('api', 'GET before any summary', [404, 'no_summary'], [r.status, r.body?.error]);
r = await call('GET', '/videos/no-such-video/summary');
check('api', 'GET for an unknown video', [404, 'no_video'], [r.status, r.body?.error]);
r = await call('POST', path, {}, { Origin: 'https://www.youtube.com' });
check('api', 'POST from a site origin is refused', 403, r.status);
const made = await call('POST', path, {});
check('api', 'POST make: summarized by one call', [200, false, true], [made.status, made.body?.reused, made.body?.markdown?.startsWith('## 한 줄 요약\n\n')]);
check('api', 'POST make: six template sections', ['한 줄 요약', '핵심 내용', '다루는 주제', '난이도', '먼저 알면 좋은 것', '다음에 볼 주제'],
  [...(made.body?.markdown ?? '').matchAll(/^## (.+)$/gm)].map((m) => m[1]));
const one = made.body?.cost_krw;
check('api', 'POST make: a cost was metered', true, one > 0, one > 0);
let after = await job();
check('api', 'P7 adds the summary cost to the video cost', [round(translated.cost_krw + one), round(one)], [round(after.cost_krw), round(after.summary_cost_krw)]);
const reused = await call('POST', path, {});
check('api', 'POST make again: the stored summary, no call', [200, true, made.body?.markdown, one], [reused.status, reused.body?.reused, reused.body?.markdown, reused.body?.cost_krw]);
r = await call('GET', path);
check('api', 'GET after: the stored summary', [200, true, made.body?.markdown], [r.status, r.body?.reused, r.body?.markdown]);
const again = await call('POST', path, { again: true });
check('api', 'POST again: a second call, its cost added', [200, false, round(2 * one)], [again.status, again.body?.reused, round(again.body?.cost_krw)]);
after = await job();
check('api', 'P7 after again', round(translated.cost_krw + 2 * one), round(after.cost_krw));

// ---- Browser: both window sizes
const browser = await chromium.launch();
const INVALID = { error: 'summary_invalid', message: '요약 답이 규칙에 맞지 않아 저장하지 않았습니다. 다시 시도해 주세요.' };
const HEADINGS = ['한 줄 요약', '핵심 내용', '다루는 주제', '난이도', '먼저 알면 좋은 것', '다음에 볼 주제'];
for (const [w, h] of [[1280, 720], [384, 854]]) {
  const size = `${w}x${h}`;
  const shot = async (page, state) => { await page.waitForTimeout(300); await page.screenshot({ path: join(out, `${size}-${state}.png`), fullPage: true }); };
  // P19 answered in the page: GET as "none stored yet", POST by `post`.
  const routed = async (post) => {
    const page = await browser.newPage({ viewport: { width: w, height: h } });
    await page.route('**/videos/*/summary', (route) => route.request().method() === 'GET'
      ? route.fulfill({ status: 404, contentType: 'application/json', body: '{"error":"no_summary"}' }) : post(route));
    await page.goto(base + '/');
    await page.getByRole('button', { name: '요약', exact: true }).waitFor();
    return page;
  };
  let page = await routed(() => {});
  const section = page.locator('[data-feature="video-summary"]');
  // The player frame and every visible status chip end above the section.
  const above = [await page.locator('.video-stage').boundingBox(),
    ...await Promise.all((await page.locator('[data-feature="job-progress"]:visible').all()).map((l) => l.boundingBox()))];
  const box = await section.boundingBox();
  const below = above.length > 1 && above.every((b) => b && box && box.y >= b.y + b.height - 1);
  check(size, 'button: 요약 is offered below the player and its status', true, below, below);
  await shot(page, 'button');
  await page.getByRole('button', { name: '요약', exact: true }).click();
  await page.getByRole('button', { name: '요약하는 중' }).waitFor();
  check(size, 'loading: the button is disabled and the section busy', [true, 'true'],
    [await page.getByRole('button', { name: '요약하는 중' }).isDisabled(), await section.getAttribute('aria-busy')]);
  await shot(page, 'loading');
  await page.close();

  page = await routed((route) => route.fulfill({ status: 502, contentType: 'application/json', body: JSON.stringify(INVALID) }));
  await page.getByRole('button', { name: '요약', exact: true }).click();
  const alert = page.getByRole('alert').filter({ hasText: '요약 답이' });
  await alert.waitFor();
  check(size, 'error: the message and 다시 시도', [true, true], [(await alert.innerText()).includes(INVALID.message), await alert.getByRole('button', { name: '다시 시도' }).isVisible()]);
  await shot(page, 'error');
  await page.close();

  // Through the backend: the summary stored above is shown with no POST.
  page = await browser.newPage({ viewport: { width: w, height: h } });
  const posts = [];
  page.on('request', (q) => { if (q.method() === 'POST' && q.url().endsWith('/summary')) posts.push(q.url()); });
  await page.goto(base + '/');
  await page.getByRole('button', { name: '다시 요약' }).waitFor();
  const s = page.locator('[data-feature="video-summary"]');
  check(size, 'summary: the stored summary as six headings, no POST', [HEADINGS, 0], [await s.locator('h3').allInnerTexts(), posts.length]);
  check(size, 'summary: its cost line', true, (await s.innerText()).includes('요약 비용'), (await s.innerText()).includes('요약 비용'));
  await shot(page, 'summary');
  await page.close();

  // Through the backend and the stand-in Gemini: 다시 요약 pressed, the screen left while
  // the call is out, the video opened again. Every page load records the commit its
  // screens were built at and the bytes of the scripts and styles it was served.
  page = await browser.newPage({ viewport: { width: w, height: h } });
  const heads = [];
  const assets = [];
  page.on('response', (q) => {
    const p = new URL(q.url()).pathname;
    if (!q.url().startsWith(base) || !/\.(js|css)$/.test(p)) return;
    q.body().then((b) => assets.push([p, sha(b), sha(readFileSync(join('dist', p)))]), () => assets.push([p, 'unread', '']));
  });
  const load = async () => { await page.goto(base + '/'); heads.push(await page.locator('meta[name="wiki-build-head"]').getAttribute('content')); };
  await load();
  await page.getByRole('button', { name: '다시 요약' }).waitFor();
  const spent = (await job()).summary_cost_krw;
  const sent = page.waitForRequest((q) => q.method() === 'POST' && q.url().endsWith('/summary'));
  await page.getByRole('button', { name: '다시 요약' }).click();
  await sent;
  await page.goto(base + '/preferences');
  const reread = page.waitForResponse((q) => q.request().method() === 'GET' && q.url().endsWith('/summary'));
  const reposts = [];
  page.on('request', (q) => { if (q.method() === 'POST' && q.url().endsWith('/summary')) reposts.push(q.url()); });
  await load();
  const first = await (await reread).json().catch(() => null);
  const waiting = await page.getByRole('button', { name: '요약하는 중' }).waitFor({ timeout: 2000 }).then(() => true, () => false);
  check(size, 'reopened while the call is out: the first read says pending and the screen waits', [true, true], [first?.pending === true, waiting]);
  await shot(page, 'reopened');
  await page.getByRole('button', { name: '다시 요약' }).waitFor({ timeout: 15000 });
  const r2 = page.locator('[data-feature="video-summary"]');
  check(size, 'reopened: then shows the summary the call stored, with no POST of its own', [HEADINGS, 0], [await r2.locator('h3').allInnerTexts(), reposts.length]);
  check(size, 'reopened: the call that outlived its screen was metered once', round(spent + one), round((await job()).summary_cost_krw));
  check(size, 'screens: every page was built at the server head', heads.map(() => build?.head), heads);
  check(size, 'screens: every script and style served is dist/ as built', true, assets.length > 0 && assets.every(([, got, built]) => got === built), assets.length > 0 && assets.every(([, got, built]) => got === built));
  await page.close();
}
await browser.close();

const report = {
  build,
  checkout,
  screens: { built_at: readFileSync(join('dist', 'index.html'), 'utf8').match(/name="wiki-build-head" content="([^"]*)"/)?.[1] ?? null },
  environment: { node: process.version, platform: `${process.platform} ${process.arch}`, playwright: JSON.parse(readFileSync('node_modules/playwright/package.json', 'utf8')).version },
  taken_at: new Date().toISOString(),
  video_key: key,
  checks,
};
writeFileSync(join(out, 'report.json'), JSON.stringify(report, null, 2) + '\n');
console.log(`${checks.filter((c) => c.pass).length}/${checks.length} checks passed at head ${build?.head}`);
process.exit(checks.every((c) => c.pass) ? 0 : 1);
