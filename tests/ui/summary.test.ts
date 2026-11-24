// S1's 요약 (P19): offered only once a video's subtitles are ready; loading,
// the rendered summary, an error with retry, a stored summary shown again,
// and a reply that arrives after the screen was left.
import { flushSync } from 'svelte';
import { expect, test, vi } from 'vitest';
import '../../src/ui/app.css';
import { go } from '../../src/ui/router.svelte';
import { blocks } from '../../src/ui/Summary.svelte';
import { $, $$, api, button, calls, features, open, text, type Answer, type Call } from './fake';

const FILE = 'https://media.example.org/lectures/attention.mp4';
const job = (stage: string) => ({ job_id: 'j', video_key: 'vk', stage, percent: 100, cost_krw: 12.5, failure_reason: null });
const CUES = { video_key: 'vk', duration_ms: 60_000, source_language: 'en', cues: [{ seq: 0, start_ms: 0, end_ms: 4_000, text_ko: '어텐션입니다.' }] };
const MARKDOWN = '## 한 줄 요약\n\n어텐션을 설명합니다.\n\n## 핵심 내용\n\n- 첫째\n- 둘째\n\n## 다루는 주제\n\n- attention\n\n## 난이도\n\n중급\n\n## 먼저 알면 좋은 것\n\n없음\n\n## 다음에 볼 주제\n\n- C\\+\\+ \\*pointers\\*\n';
const MADE = { video_key: 'vk', summary: {}, markdown: MARKDOWN, cost_krw: 3.2, reused: false };

/** A ready video at FILE; `summary` answers P19, `subtitles` P8. */
function serve(summary: (c: Call) => Answer | Promise<Answer>, stage = 'ready', subtitles: Answer = [200, CUES]) {
  api((c) => {
    if (c.path === '/browser') return [200, { url: FILE, job: job(stage) }];
    if (c.path === '/jobs') return [200, job(stage)];
    if (c.path.endsWith('/summary')) return summary(c);
    if (c.path === '/library/data') return [200, { videos: [], playlists: [], legacy_available: false }];
    return subtitles;
  });
}

const summaryCalls = () => calls.filter((c) => c.path === '/videos/vk/summary');
const section = (root: HTMLElement) => $(root, '[data-feature="video-summary"]')!;

test('the Markdown is read as headings, lists and paragraphs of text, unescaped, never as HTML', () => {
  expect(blocks('## 제목\n\n<b>문단</b>\n- 하나\n- \\- 둘\n\n## 다음\n\n없음\n')).toEqual([
    { kind: 'heading', text: '제목' },
    { kind: 'paragraph', text: '<b>문단</b>' },
    { kind: 'list', items: ['하나', '- 둘'] },
    { kind: 'heading', text: '다음' },
    { kind: 'paragraph', text: '없음' },
  ]);
});

test('a ready video offers 요약; pressing it shows the loading state, then the rendered summary and its cost', async () => {
  let answer: ((a: Answer) => void) | undefined;
  serve((c) => (c.method === 'GET' ? [404, { error: 'no_summary' }] : new Promise((r) => (answer = r))));
  const s1 = open('/');
  await vi.waitFor(() => expect(button(s1, '요약')).toBeTruthy());
  expect(features(s1)).toContain('video-summary');
  button(s1, '요약').click();
  await vi.waitFor(() => expect(button(s1, '요약하는 중').disabled).toBe(true));
  expect(section(s1).getAttribute('aria-busy')).toBe('true');
  expect(text($(section(s1), '[role="status"]')!)).toContain('화면을 떠나도 요약은 저장되고');
  answer!([200, MADE]);
  await vi.waitFor(() => expect($$(section(s1), 'h3').map(text)).toEqual(['한 줄 요약', '핵심 내용', '다루는 주제', '난이도', '먼저 알면 좋은 것', '다음에 볼 주제']));
  expect($$(section(s1), 'li').map(text)).toEqual(['첫째', '둘째', 'attention', 'C++ *pointers*']);
  expect(text(section(s1))).toContain('요약 비용 3.2원');
  expect(summaryCalls().map((c) => [c.method, c.body])).toEqual([['GET', undefined], ['POST', {}]]);
  // Re-summarizing is its own button, and says so in its request.
  button(s1, '다시 요약').click();
  await vi.waitFor(() => expect(summaryCalls().at(-1)!.body).toEqual({ again: true }));
});

test('a stored summary is shown on opening, with no POST', async () => {
  serve((c) => (c.method === 'GET' ? [200, { ...MADE, reused: true }] : [500, {}]));
  const s1 = open('/');
  await vi.waitFor(() => expect(text(section(s1))).toContain('어텐션을 설명합니다.'));
  expect(summaryCalls().every((c) => c.method === 'GET')).toBe(true);
  expect(() => button(s1, '요약')).toThrow();
});

// The property: 요약 is offered exactly when the video's subtitles play, whatever the latest job's stage.
const NOT_READY: Answer = [409, { error: 'not_ready', message: '자막을 만드는 중입니다.' }];
test.each([
  { name: 'a translating video', stage: 'translating', subtitles: NOT_READY, offered: false },
  { name: 'a failed first translation, with no subtitles', stage: 'failed', subtitles: NOT_READY, offered: false },
  { name: 'a failed re-make, whose saved subtitles still play', stage: 'failed', subtitles: [200, CUES] as Answer, offered: true },
])('$name: 요약 offered $offered', async ({ stage, subtitles, offered }) => {
  serve(() => [200, { ...MADE, reused: true }], stage, subtitles);
  const s1 = open('/');
  await vi.waitFor(() => expect(features(s1)).toContain('job-progress'));
  if (offered) {
    await vi.waitFor(() => expect(text(section(s1))).toContain('어텐션을 설명합니다.'));
  } else {
    await new Promise((r) => setTimeout(r, 200));
    expect(features(s1)).not.toContain('video-summary');
    expect(summaryCalls()).toEqual([]);
  }
});

// The property: wherever the video's subtitles play, the overlay shows the video's cost as P7 reports
// it, the summary's included, and follows it when a summary call adds to it.
test.each([
  { name: 'a ready video', stage: 'ready' },
  { name: 'a failed re-make, whose saved subtitles still play', stage: 'failed' },
])('$name: the overlay shows the video cost with the summary added', async ({ stage }) => {
  let cost = 12.5;
  // The summary lookup answers only after the cost is on screen, so the button comes after it.
  let lookedUp: (a: Answer) => void = () => {};
  api((c) => {
    if (c.path === '/browser') return [200, { url: FILE, job: { ...job(stage), cost_krw: cost } }];
    if (c.path === '/jobs') return [200, { ...job(stage), cost_krw: cost }];
    if (c.path.endsWith('/summary')) return c.method === 'GET' ? new Promise((r) => (lookedUp = r)) : ((cost = 15.7), [200, MADE]);
    if (c.path === '/library/data') return [200, { videos: [], playlists: [], legacy_available: false }];
    return [200, CUES];
  });
  const s1 = open('/');
  const status = () => text($(s1, '[data-feature="job-progress"]')!);
  await vi.waitFor(() => expect(status()).toContain('12.5원'));
  await vi.waitFor(() => expect(summaryCalls().map((c) => c.method)).toEqual(['GET']));
  lookedUp([404, { error: 'no_summary' }]);
  await vi.waitFor(() => expect(button(s1, '요약')).toBeTruthy());
  button(s1, '요약').click();
  await vi.waitFor(() => expect(text(section(s1))).toContain('요약 비용 3.2원'));
  await vi.waitFor(() => expect(status()).toContain('15.7원'), { timeout: 3_000 });
});

test('an untranslated link (no job) does not offer 요약', async () => {
  api((c) => (c.path === '/browser' ? [200, { url: FILE, job: null }] : [404, { error: 'no_job' }]));
  const s1 = open('/');
  await vi.waitFor(() => expect(text(s1)).toContain('작업 전'));
  expect(features(s1)).not.toContain('video-summary');
  expect(summaryCalls()).toEqual([]);
});

test('a reply that breaks a rule shows the error with 다시 시도, which asks again', async () => {
  const replies: Answer[] = [[502, { error: 'summary_invalid', message: '요약 답이 규칙에 맞지 않아 저장하지 않았습니다. 다시 시도해 주세요.' }], [200, MADE]];
  serve((c) => (c.method === 'GET' ? [404, { error: 'no_summary' }] : replies.shift()!));
  const s1 = open('/');
  await vi.waitFor(() => expect(button(s1, '요약')).toBeTruthy());
  button(s1, '요약').click();
  await vi.waitFor(() => expect(text($(section(s1), '[role="alert"]')!)).toContain('요약 답이 규칙에 맞지 않아 저장하지 않았습니다.'));
  expect(button(s1, '요약').disabled).toBe(false);
  button(s1, '다시 시도').click();
  await vi.waitFor(() => expect(text(section(s1))).toContain('어텐션을 설명합니다.'));
  expect($(section(s1), '[role="alert"]')).toBeNull();
  expect(summaryCalls().filter((c) => c.method === 'POST')).toHaveLength(2);
});

test('a refused 다시 요약 keeps the summary and shows its cost with the billed reply added', async () => {
  let cost = 3.2;
  serve((c) => {
    if (c.method === 'GET') return [200, { ...MADE, cost_krw: cost, reused: true }];
    cost = 4.1;
    return [502, { error: 'summary_invalid', message: '요약 답이 규칙에 맞지 않아 저장하지 않았습니다. 다시 시도해 주세요.' }];
  });
  const s1 = open('/');
  await vi.waitFor(() => expect(text(section(s1))).toContain('요약 비용 3.2원'));
  button(s1, '다시 요약').click();
  await vi.waitFor(() => expect(text(section(s1))).toContain('요약 비용 4.1원'));
  expect(text($(section(s1), '[role="alert"]')!)).toContain('저장하지 않았습니다');
  expect(text(section(s1))).toContain('어텐션을 설명합니다.');
});

test('a reply that arrives after the screen was left is dropped there and shown when the video is opened again', async () => {
  let answer: ((a: Answer) => void) | undefined;
  let stored = false;
  serve((c) => {
    if (c.method === 'GET') return stored ? [200, { ...MADE, reused: true }] : [404, { error: 'no_summary' }];
    return new Promise((r) => (answer = (a) => { stored = true; r(a); }));
  });
  const s1 = open('/');
  await vi.waitFor(() => expect(button(s1, '요약')).toBeTruthy());
  button(s1, '요약').click();
  await vi.waitFor(() => expect(answer).toBeTruthy());
  go('/library');
  flushSync();
  expect($(s1, '[data-feature="video-summary"]')).toBeNull();
  answer!([200, MADE]);
  await new Promise((r) => setTimeout(r, 50));
  go('/');
  flushSync();
  await vi.waitFor(() => expect(text(section(s1))).toContain('어텐션을 설명합니다.'));
  expect(summaryCalls().filter((c) => c.method === 'POST')).toHaveLength(1);
});

// The property: a screen reopened while a call is out waits for it, over a stored summary or none,
// then shows what the call did: the new summary, or the error with 다시 시도 beside what was stored.
const NEWER = { ...MADE, markdown: MARKDOWN.replace('어텐션을 설명합니다.', '어텐션을 다시 설명합니다.'), cost_krw: 6.4 };
const INVALID = '요약 답이 규칙에 맞지 않아 저장하지 않았습니다. 다시 시도해 주세요.';
test.each([
  { name: 'a first summary', before: null, answer: [200, MADE], shows: '어텐션을 설명합니다.', cost: '3.2원', failed: false },
  { name: 'a first summary whose reply breaks a rule', before: null, answer: [502, { error: 'summary_invalid', message: INVALID }], shows: null, cost: null, failed: true },
  { name: 'a re-summary', before: MADE, answer: [200, NEWER], shows: '어텐션을 다시 설명합니다.', cost: '6.4원', failed: false },
  { name: 'a re-summary whose reply breaks a rule', before: MADE, answer: [502, { error: 'summary_invalid', message: INVALID }], shows: '어텐션을 설명합니다.', cost: '3.2원', failed: true },
] as const)('reopened during $name: waits, then shows what the call did', async ({ before, answer, shows, cost, failed }) => {
  // The backend as P19 behaves: a read says pending while the call is out, beside any stored summary.
  let stored: any = before;
  let failure: string | null = null;
  let release: (() => void) | undefined;
  serve((c) => {
    if (c.method === 'GET') {
      const pending = release ? { pending: true } : failure ? { failure } : {};
      return stored ? [200, { ...stored, reused: true, ...pending }] : release || failure ? [200, { video_key: 'vk', cost_krw: 0, ...pending }] : [404, { error: 'no_summary' }];
    }
    return new Promise((r) => (release = () => {
      if (answer[0] === 200) stored = answer[1];
      else failure = INVALID;
      release = undefined;
      r(answer as Answer);
    }));
  });
  const s1 = open('/');
  await vi.waitFor(() => expect(button(s1, before ? '다시 요약' : '요약')).toBeTruthy());
  button(s1, before ? '다시 요약' : '요약').click();
  await vi.waitFor(() => expect(release).toBeTruthy());
  go('/library');
  flushSync();
  go('/');
  flushSync();
  await vi.waitFor(() => expect(button(s1, '요약하는 중').disabled).toBe(true));
  release!();
  if (failed) {
    await vi.waitFor(() => expect(text($(section(s1), '[role="alert"]')!)).toContain('저장하지 않았습니다'), { timeout: 4000 });
    expect(button(s1, '다시 시도')).toBeTruthy();
  } else {
    await vi.waitFor(() => expect(text(section(s1))).toContain(shows!), { timeout: 4000 });
    expect($(section(s1), '[role="alert"]')).toBeNull();
  }
  if (shows) expect(text(section(s1))).toContain(shows);
  if (cost) expect(text(section(s1))).toContain(`요약 비용 ${cost}`);
  expect(summaryCalls().filter((c) => c.method === 'POST')).toHaveLength(1);
});

// The property: a read that gets no answer (a 5xx, or the app unreachable) says nothing about the
// summary. A screen reopened during a call keeps waiting, asks again, and shows what the call did
// with no user action, wherever the failed read falls.
test.each([
  { name: 'a 503 on a poll while waiting', at: 2, status: 503 },
  { name: 'a 500 on a poll while waiting', at: 2, status: 500 },
  { name: 'an unreachable app on a poll while waiting', at: 2, status: 0 },
  { name: 'a 503 on the first read after reopening', at: 1, status: 503 },
])('reopened during a call, $name: keeps waiting, then shows the summary', async ({ at, status }) => {
  let stored: any = null;
  let release: (() => void) | undefined;
  let reads = -1; // reads since reopening, -1 before
  serve((c) => {
    if (c.method === 'GET') {
      if (reads >= 0 && ++reads === at) {
        if (status === 0) throw new TypeError('network down');
        return [status, { error: 'temporary' }];
      }
      return stored ? [200, { ...stored, reused: true }] : release ? [200, { video_key: 'vk', cost_krw: 0, pending: true }] : [404, { error: 'no_summary' }];
    }
    return new Promise((r) => (release = () => { stored = MADE; release = undefined; r([200, MADE]); }));
  });
  const s1 = open('/');
  await vi.waitFor(() => expect(button(s1, '요약')).toBeTruthy());
  button(s1, '요약').click();
  await vi.waitFor(() => expect(release).toBeTruthy());
  go('/library');
  flushSync();
  reads = 0;
  go('/');
  flushSync();
  // A read after the failed one, made by the screen itself.
  await vi.waitFor(() => expect(reads).toBeGreaterThan(at), { timeout: 4000 });
  await vi.waitFor(() => expect(button(s1, '요약하는 중').disabled).toBe(true));
  release!();
  await vi.waitFor(() => expect(text(section(s1))).toContain('어텐션을 설명합니다.'), { timeout: 4000 });
  expect($(section(s1), '[role="alert"]')).toBeNull();
  expect(summaryCalls().filter((c) => c.method === 'POST')).toHaveLength(1);
});

test('without a key, the error says to add one in the settings', async () => {
  serve((c) => (c.method === 'GET' ? [404, {}] : [428, { error: 'no_key' }]));
  const s1 = open('/');
  await vi.waitFor(() => expect(button(s1, '요약')).toBeTruthy());
  button(s1, '요약').click();
  await vi.waitFor(() => expect(text($(section(s1), '[role="alert"]')!)).toContain('설정에서 Gemini 개인키를 넣어 주세요'));
});
