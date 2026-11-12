// S1 `/`: its states (empty, loading, error, full), opening an address, the
// YouTube account screen, 영상감지 and its refusals, and a job's progress to
// its ready subtitles.
import { expect, test, vi } from 'vitest';
import { $, api, button, calls, features, open, text, type } from './fake';

const YT = 'https://www.youtube.com/watch?v=kCc8FmEb1nY';
const FILE = 'https://media.example.org/lectures/attention.mp4';

test('empty: only the address bar and where the video will open', async () => {
  api(() => [200, { url: null, job: null }]);
  const s1 = open('/');
  await vi.waitFor(() => expect(text(s1)).toContain('영상 주소를 넣으면 여기에서 열립니다'));
  expect(features(s1)).toEqual(['open-site']);
  expect(button(s1, '설정').getAttribute('href')).toBe('/preferences');
});

test('loading: the overlay is sketched until P9 answers', async () => {
  api(() => 'hang');
  const s1 = open('/');
  expect($(s1, '[aria-busy="true"]')).not.toBeNull();
  expect(features(s1)).toEqual(['detect-video', 'job-progress', 'open-site']);
  expect((s1.querySelector('[data-feature="detect-video"]') as HTMLButtonElement).disabled).toBe(true);
});

test('error: a failed P9 says so and loads again', async () => {
  let fail = true;
  api(() => (fail ? [500, {}] : [200, { url: null, job: null }]));
  const s1 = open('/');
  await vi.waitFor(() => expect(text(s1)).toContain('화면을 불러오지 못했습니다'));
  fail = false;
  button(s1, '다시 불러오기').click();
  await vi.waitFor(() => expect(text(s1)).toContain('영상 주소를 넣으면 여기에서 열립니다'));
});

test('an address that cannot be opened is told under the address bar; one that can opens with 영상감지 over it', async () => {
  let opened: string | null = null;
  api((c) => {
    if (c.path === '/browser') return [200, { url: opened, job: null }];
    if (c.body.url.startsWith('https://')) {
      opened = c.body.url;
      return [200, { opened_url: opened, site: 'other', account_gate: false }];
    }
    return [400, { error: 'bad_url', message: 'http나 https 주소만 열 수 있습니다.' }];
  });
  const s1 = open('/');
  await vi.waitFor(() => expect(text(s1)).toContain('여기에서 열립니다'));
  type($(s1, '#address'), 'javascript:alert(1)');
  button(s1, '열기').click();
  await vi.waitFor(() => expect(text($(s1, '#address-error')!)).toContain('열 수 없는 주소입니다'));

  type($(s1, '#address'), FILE);
  button(s1, '열기').click();
  await vi.waitFor(() => expect($(s1, 'video')?.getAttribute('src')).toBe(FILE));
  expect($(s1, '#address-error')).toBeNull();
  // Every feature of the first screen is on it before any job: where progress and subtitles will show.
  expect(features(s1)).toEqual(['detect-video', 'job-progress', 'open-site', 'watch-subtitles']);
  expect(text($(s1, '[data-feature="job-progress"]')!)).toBe('진행 상태 작업 전 든 비용 0원');
  expect(text($(s1, '[data-feature="watch-subtitles"]')!)).toBe('한국어 자막이 여기에 뜹니다');
  // P9 again after P1: the page's job comes with it, so P7 is not asked for a page that has none.
  expect(calls.map((c) => `${c.method} ${c.path}`)).toEqual(['GET /browser', 'POST /browser/open', 'POST /browser/open', 'GET /browser']);
});

test("YouTube's first open shows Google's account screen, and it can be skipped", async () => {
  const gate = `https://accounts.google.com/ServiceLogin?service=youtube&continue=${encodeURIComponent(YT)}`;
  let opened = gate;
  api((c) => {
    if (c.path === '/browser') return [200, { url: opened, job: null }];
    opened = c.body.url;
    return [200, { opened_url: opened, site: 'youtube', account_gate: false }];
  });
  const s1 = open('/');
  await vi.waitFor(() => expect(text(s1)).toContain('유튜브 계정 화면'));
  expect(button(s1, '구글 로그인 화면 열기').getAttribute('href')).toBe(gate);
  button(s1, '유튜브 영상으로 가기').click();
  await vi.waitFor(() => expect(text(s1)).toContain('유튜브 영상 재생'));
  expect(calls.find((c) => c.method === 'POST')!.body).toEqual({ url: YT });
});

test('영상감지 without a key: the reason, and one click to the settings', async () => {
  api((c) => {
    if (c.path === '/browser') return [200, { url: FILE, job: null }];
    if (c.path === '/jobs') return [428, { error: 'no_key', message: '설정에서 Gemini API 키를 먼저 넣어 주세요.' }];
    return [200, { key_set: false, key_masked: null, translation_model: 'gemini-3.1-flash-lite' }];
  });
  const s1 = open('/');
  await vi.waitFor(() => expect($(s1, 'video')).not.toBeNull());
  // A media file's length is the player's; here none loads, so it is sent as P6 refuses it, and the reply is shown.
  Object.defineProperty($(s1, 'video')!, 'duration', { value: 61.5 });
  $(s1, 'video')!.dispatchEvent(new Event('durationchange'));
  button(s1, '영상감지').click();
  await vi.waitFor(() => expect(text(s1)).toContain('개인키가 없어 작업을 시작하지 않았습니다'));
  const p6 = calls.find((c) => c.method === 'POST' && c.path === '/jobs')!;
  expect(p6.body).toEqual({ page_url: FILE, media_url: FILE, title: '', duration_ms: 61_500 });
  button(s1, '설정으로 가기').click();
  await vi.waitFor(() => expect(text(s1)).toContain('Gemini 개인키'));
  expect(location.pathname).toBe('/preferences');
});

test('another job running, or audio that cannot be had: the reason is shown and 영상감지 stays', async () => {
  const replies: [number, unknown][] = [
    [409, { error: 'busy', message: '다른 영상의 작업이 돌고 있습니다. 그 작업이 끝난 뒤에 다시 눌러 주세요.' }],
    [422, { error: 'drm', message: 'DRM으로 보호된 영상은 내려받을 수 없습니다.' }],
  ];
  api((c) => (c.path === '/browser' ? [200, { url: FILE, job: null }] : replies.shift()!));
  const s1 = open('/');
  await vi.waitFor(() => expect($(s1, 'video')).not.toBeNull());
  Object.defineProperty($(s1, 'video')!, 'duration', { value: 10 });
  $(s1, 'video')!.dispatchEvent(new Event('durationchange'));
  for (const reason of ['그 작업이 끝난 뒤에 다시 눌러 주세요', 'DRM으로 보호된 영상은 내려받을 수 없습니다']) {
    button(s1, '영상감지').click();
    await vi.waitFor(() => expect(text(s1)).toContain(reason));
    expect(features(s1)).toContain('detect-video');
  }
});

test('a running job shows its stage, percent and cost each second, then its subtitles are ready', async () => {
  const job = (stage: string, percent: number, cost: number) => ({ job_id: 'j', video_key: 'vk', stage, percent, cost_krw: cost, failure_reason: null });
  const looks = [job('translating', 70, 8.25), job('ready', 100, 12.5)];
  api((c) => {
    if (c.path === '/browser') return [200, { url: FILE, job: job('writing', 40, 0) }];
    if (c.path === '/jobs') return [200, looks.shift() ?? job('ready', 100, 12.5)];
    return [200, { video_key: 'vk', duration_ms: 60_000, source_language: 'en', cues: [{ seq: 1, start_ms: 0, end_ms: 4_000, text_ko: '어텐션이 전부입니다' }] }];
  });
  const s1 = open('/');
  await vi.waitFor(() => expect(text(s1)).toMatch(/진행 상태 ?write 중 ?40% ?든 비용 0원/));
  // Subtitles wait for the whole job (R11), and 영상감지 stays on the overlay, off, while it runs.
  expect(features(s1)).toEqual(['detect-video', 'job-progress', 'open-site', 'watch-subtitles']);
  expect(button(s1, '영상감지').disabled).toBe(true);
  expect(text($(s1, '[data-feature="watch-subtitles"]')!)).toBe('자막은 작업이 다 끝나면 여기에 뜹니다');
  await vi.waitFor(() => expect(text(s1)).toMatch(/번역 중 ?70% ?든 비용 8.3원/), { timeout: 3_000 });
  await vi.waitFor(() => expect(text(s1)).toContain('자막 준비됨 · 12.5원'), { timeout: 3_000 });
  // On a ready video 영상감지 is on again: it shows the saved subtitles without making them again (R25).
  expect(button(s1, '영상감지').disabled).toBe(false);
  expect(calls.filter((c) => c.path === '/jobs').every((c) => c.query.page_url === FILE)).toBe(true);
  expect(calls.at(-1)!.path).toBe('/videos/vk/subtitles');
});

test('a job that stopped shows why and offers 영상감지 again', async () => {
  api(() => [200, { url: FILE, job: { job_id: 'j', video_key: 'vk', stage: 'failed', percent: 30, cost_krw: 1, failure_reason: 'no_speech' } }]);
  const s1 = open('/');
  await vi.waitFor(() => expect(text(s1)).toContain('작업이 멈췄습니다. 영상에서 말소리를 찾지 못했습니다.'));
  expect(features(s1)).toEqual(['detect-video', 'job-progress', 'open-site', 'watch-subtitles']);
});

const READY = { url: FILE, job: { job_id: 'j', video_key: 'vk', stage: 'ready', percent: 100, cost_krw: 0.4, failure_reason: null } };

test("a ready video's subtitles while they load: 자막 준비됨 at once, the subtitle line says they are coming, 영상감지 off", async () => {
  api((c) => (c.path === '/browser' ? [200, READY] : 'hang'));
  const s1 = open('/');
  await vi.waitFor(() => expect(text($(s1, '[data-feature="watch-subtitles"]')!)).toBe('자막을 불러오는 중'));
  expect(text($(s1, '[data-feature="job-progress"]')!)).toBe('자막 준비됨 · 0.4원');
  expect(button(s1, '영상감지').disabled).toBe(true);
});

test("a ready video's subtitles that fail to load: the reason, and 영상감지 brings them back", async () => {
  const CUES = { video_key: 'vk', duration_ms: 60_000, source_language: 'en', cues: [{ seq: 0, start_ms: 0, end_ms: 4_000, text_ko: '어텐션만 있으면 됩니다.' }] };
  let subtitles: [number, unknown] = [500, {}];
  api((c) => {
    if (c.path === '/browser') return [200, READY];
    if (c.path === '/jobs' && c.method === 'POST') return [200, { job_id: 'j', video_key: 'vk', stage: 'ready', reused: true }];
    if (c.path === '/jobs') return [200, READY.job];
    return subtitles;
  });
  const s1 = open('/');
  await vi.waitFor(() => expect(text($(s1, '[role="alert"]')!)).toBe('저장해 둔 자막을 읽지 못했습니다. 영상감지 버튼을 다시 눌러 주세요.'));
  subtitles = [200, CUES];
  Object.defineProperty($(s1, 'video')!, 'duration', { value: 60 });
  $(s1, 'video')!.dispatchEvent(new Event('durationchange'));
  button(s1, '영상감지').click();
  await vi.waitFor(() => expect(text($(s1, '[data-feature="watch-subtitles"]')!)).toBe('어텐션만 있으면 됩니다.'));
  expect(text(s1)).toContain('자막 준비됨 · 0.4원');
});
