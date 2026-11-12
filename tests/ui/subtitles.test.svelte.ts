// The overlay's subtitle follows the player: the cue under the playback time,
// none between cues, and none while an ad (another length) plays, the
// lecture's cue back when it returns (R4, R18).
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, expect, test, vi } from 'vitest';
import Overlay from '../../src/ui/Overlay.svelte';
import { api, text } from './fake';

let shown: ReturnType<typeof mount> | null = null;
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
