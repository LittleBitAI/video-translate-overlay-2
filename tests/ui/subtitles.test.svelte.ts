// The overlay's subtitle follows the player: the cue under the playback time,
// none between cues, and none while an ad (another length) plays, the
// lecture's cue back when it returns (R4, R18).
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, expect, onTestFinished, test, vi } from 'vitest';
import Overlay from '../../src/ui/Overlay.svelte';
import { api, calls, text } from './fake';

let shown: ReturnType<typeof mount> | null = null;

const deletionCases = ['native', 'broadcast'].flatMap(source => ['ready', 'failed'].map(stage => ({source, stage})));
function deletedVideo(source: string, key: string) {
  if (source === 'native') window.dispatchEvent(new CustomEvent('vto-video-deleted', {detail: key}));
  else {
    const channel = new BroadcastChannel('vto-library');
    onTestFinished(() => channel.close());
    channel.postMessage(key);
  }
}

test.each(deletionCases)('deleting another video preserves cues through a failed refresh: $source $stage', async ({source, stage}) => {
  api(c => c.path === '/jobs' ? [500, {message: 'Unavailable'}] : [200, {
    duration_ms: 600000, cues: [{seq: 0, start_ms: 1000, end_ms: 4000, text_ko: 'Unrelated saved cue'}]}]);
  const target = document.body.appendChild(document.createElement('div'));
  shown = mount(Overlay, {target, props: {pageUrl: 'https://talks.example.org/7', timeMs: 1500, durationMs: 600000,
    initial: {stage, percent: 100, cost_krw: 3, failure_reason: null, video_key: 'vk'}}});
  await vi.waitFor(() => expect(text(target)).toContain('Unrelated saved cue'));
  deletedVideo(source, 'other-video');
  await vi.waitFor(() => expect(calls.some(c => c.path === '/jobs')).toBe(true), {timeout: 2500});
  expect(text(target)).toContain('Unrelated saved cue');
  expect(calls.filter(c => c.path === '/videos/vk/subtitles')).toHaveLength(1);
  expect(calls.filter(c => c.method === 'POST')).toHaveLength(0);
});

test.each(deletionCases)('a late subtitle response after deletion cannot restore cached cues: $source $stage', async ({source, stage}) => {
  let reply: (a: any) => void=()=>{};
  api(c => c.path === '/jobs' ? [410,{message:'Deleted',deletion_token:'new-token'}]
    : new Promise(resolve=>{reply=resolve;}));
  const target=document.body.appendChild(document.createElement('div'));
  shown=mount(Overlay,{target,props:{pageUrl:'https://talks.example.org/7',timeMs:1500,durationMs:600000,
    initial:{stage,percent:100,cost_krw:3,failure_reason:null,video_key:'vk'}}});
  await vi.waitFor(()=>expect(calls.some(c=>c.path==='/videos/vk/subtitles')).toBe(true));
  deletedVideo(source, 'vk');
  await vi.waitFor(()=>expect(text(target)).toContain('Deleted'));
  reply([200,{duration_ms:600000,cues:[{seq:0,start_ms:1000,end_ms:4000,text_ko:'Deleted subtitle'}]}]);
  await new Promise(resolve=>setTimeout(resolve,50));
  expect(text(target)).not.toContain('Deleted subtitle');
  expect(calls.filter(c=>c.method==='POST')).toHaveLength(0);
});

test.each(['native', 'broadcast'])('deletion before the first job response cannot revive that video: %s', async source => {
  let resolveJob: (answer: any) => void = () => {};
  let first = true;
  api(c => {
    if (c.path === '/jobs') {
      if (first) { first = false; return new Promise(resolve => { resolveJob = resolve; }); }
      return [410, {message: 'Deleted', deletion_token: 'new-token'}];
    }
    return [200, {duration_ms: 600000, cues: [{seq: 0, start_ms: 1000, end_ms: 4000, text_ko: 'Deleted subtitle'}]}];
  });
  const target = document.body.appendChild(document.createElement('div'));
  shown = mount(Overlay, {target, props: {pageUrl: 'https://talks.example.org/7', timeMs: 1500, durationMs: 600000}});
  await vi.waitFor(() => expect(first).toBe(false));
  deletedVideo(source, 'vk');
  await new Promise(resolve => setTimeout(resolve, 50));
  resolveJob([200, {stage: 'ready', percent: 100, cost_krw: 3, failure_reason: null, video_key: 'vk'}]);
  await vi.waitFor(() => expect(text(target)).toContain('Deleted'), {timeout: 2500});
  expect(text(target)).not.toContain('Deleted subtitle');
  expect(calls.filter(c => c.path.endsWith('/subtitles'))).toHaveLength(0);
});

test('explicit detection after deletion can display newly created subtitles', async () => {
  let state = 'ready';
  const ready = {stage: 'ready', percent: 100, cost_krw: 3, failure_reason: null, video_key: 'vk'};
  api(c => {
    if (c.method === 'POST') { state = 'recreated'; return [200, ready]; }
    if (c.path === '/jobs') return state === 'deleted' ? [410, {message: 'Deleted', deletion_token: 'new-token'}] : [200, ready];
    return [200, {duration_ms: 600000, cues: [{seq: 0, start_ms: 1000, end_ms: 4000,
      text_ko: state === 'recreated' ? 'New saved cue' : 'Old saved cue'}]}];
  });
  vi.stubGlobal('confirm', () => true);
  const target = document.body.appendChild(document.createElement('div'));
  shown = mount(Overlay, {target, props: {pageUrl: 'https://talks.example.org/7', timeMs: 1500, durationMs: 600000, initial: ready}});
  await vi.waitFor(() => expect(text(target)).toContain('Old saved cue'));
  state = 'deleted'; deletedVideo('native', 'vk');
  await vi.waitFor(() => expect(text(target)).toContain('Deleted'));
  target.querySelector<HTMLButtonElement>('[data-feature="detect-video"]')!.click();
  await vi.waitFor(() => expect(text(target)).toContain('New saved cue'));
  const posts = calls.filter(c => c.method === 'POST');
  expect(posts).toHaveLength(1);
  expect(posts[0].body.deletion_token).toBe('new-token');
});
afterEach(() => {
  if (shown) unmount(shown);
  shown = null;
});

test('the cue under the playback time, hidden while an ad plays', async () => {
  api(() => [
    200,
    {
      video_key: 'vk',
      duration_ms: 600_000,
      cues: [
        { seq: 2, start_ms: 5_000, end_ms: 8_000, text_ko: '두 번째 문장' },
        { seq: 1, start_ms: 1_000, end_ms: 4_000, text_ko: '첫 번째 문장' },
      ],
    },
  ]);
  const player = $state({ timeMs: 0, durationMs: 600_000 });
  const target = document.body.appendChild(document.createElement('div'));
  shown = mount(Overlay, {
    target,
    props: {
      pageUrl: 'https://talks.example.org/7',
      initial: { stage: 'ready', percent: 100, cost_krw: 3, failure_reason: null, video_key: 'vk' },
      get timeMs() {
        return player.timeMs;
      },
      get durationMs() {
        return player.durationMs;
      },
    },
  });
  const sub = () => target.querySelector('[data-feature="watch-subtitles"]');
  await vi.waitFor(() => expect(text(target)).toContain('자막 준비됨'));
  const at = (ms: number, length = 600_000) => {
    player.timeMs = ms;
    player.durationMs = length;
    flushSync();
    return sub() ? text(sub()!) : null;
  };
  expect(at(500)).toBeNull();
  expect(at(1_000)).toBe('첫 번째 문장');
  expect(at(3_999)).toBe('첫 번째 문장');
  expect(at(4_500)).toBeNull();
  expect(at(6_000)).toBe('두 번째 문장');
  // A 15 s ad: the player's length is the ad's.
  expect(at(2_000, 15_000)).toBeNull();
  expect(at(6_000, 600_400)).toBe('두 번째 문장');
});

// A failed latest job does not determine whether an older completed set exists.
test.each([
  ['initial', true], ['initial', false], ['poll', true], ['poll', false],
] as const)('failed jobs recover saved subtitles and keep the failure reason: %s, saved=%s', async (entry, saved) => {
  const failed = { stage: 'failed', percent: 0, cost_krw: 3, failure_reason: 'translate_failed', video_key: 'vk' };
  api(c => {
    if (c.path === '/jobs') return [200, failed];
    if (c.path === '/videos/vk/subtitles') return saved
      ? [200, { duration_ms: 600_000, cues: [{ seq: 0, start_ms: 1_000, end_ms: 4_000, text_ko: 'Saved subtitle' }] }]
      : [409, { error: 'not_ready' }];
    return [404, {}];
  });
  const target = document.body.appendChild(document.createElement('div'));
  shown = mount(Overlay, {
    target,
    props: {
      pageUrl: 'https://talks.example.org/7', timeMs: 2_000, durationMs: 600_000,
      initial: entry === 'initial' ? failed : { ...failed, stage: 'translating', failure_reason: null },
    },
  });
  await vi.waitFor(() => expect(calls.some(c => c.path === '/videos/vk/subtitles')).toBe(true), { timeout: 3_000 });
  await vi.waitFor(() => expect(text(target)).toContain('작업이 멈췄습니다. 번역하지 못했습니다.'));
  expect(text(target.querySelector('[data-feature="watch-subtitles"]')!)).toBe(saved ? 'Saved subtitle' : '한국어 자막이 여기에 뜹니다');
  expect((target.querySelector('[data-feature="detect-video"]') as HTMLButtonElement).disabled).toBe(false);
  expect(calls.filter(c => c.method === 'POST')).toHaveLength(0);
});
