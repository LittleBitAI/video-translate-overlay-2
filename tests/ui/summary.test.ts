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

/** A ready video at FILE; `summary` answers P19. */
function serve(summary: (c: Call) => Answer | Promise<Answer>, stage = 'ready') {
  api((c) => {
    if (c.path === '/browser') return [200, { url: FILE, job: job(stage) }];
    if (c.path === '/jobs') return [200, job(stage)];
    if (c.path.endsWith('/summary')) return summary(c);
    if (c.path === '/library/data') return [200, { videos: [], playlists: [], legacy_available: false }];
    return [200, CUES];
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

test.each(['translating', 'failed'])('a %s video does not offer 요약', async (stage) => {
  serve(() => [200, MADE], stage);
  const s1 = open('/');
  await vi.waitFor(() => expect(features(s1)).toContain('job-progress'));
  await new Promise((r) => setTimeout(r, 200));
  expect(features(s1)).not.toContain('video-summary');
  expect(summaryCalls()).toEqual([]);
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

test('without a key, the error says to add one in the settings', async () => {
  serve((c) => (c.method === 'GET' ? [404, {}] : [428, { error: 'no_key' }]));
  const s1 = open('/');
  await vi.waitFor(() => expect(button(s1, '요약')).toBeTruthy());
  button(s1, '요약').click();
  await vi.waitFor(() => expect(text($(section(s1), '[role="alert"]')!)).toContain('설정에서 Gemini 개인키를 넣어 주세요'));
});
