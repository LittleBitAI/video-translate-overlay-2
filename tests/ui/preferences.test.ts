// S2 `/preferences`: its states, saving a key (checked by Gemini first, the
// value kept when it is refused), deleting it and signing out of YouTube,
// each asked once more.
import { expect, test, vi } from 'vitest';
import { $, api, button, calls, features, open, text, type } from './fake';

const NONE = { key_set: false, key_masked: null, translation_model: 'gemini-3.1-flash-lite', favorite_voice_available: false };
const KEY = 'AIzaSyA1b2C3d4E5f6G7h8I9j0K1l2M3n4OWXYZ';

test('empty: no key, why it is needed, and delete is off', async () => {
  api(() => [200, NONE]);
  const s2 = open('/preferences');
  await vi.waitFor(() => expect(text(s2)).toContain('개인키를 넣어야 영상감지 버튼이 작업을 시작합니다'));
  expect(features(s2)).toEqual(['delete-api-key', 'set-api-key', 'youtube-logout']);
  expect(button(s2, '개인키 지우기').disabled).toBe(true);
  expect(text(s2)).toContain('gemini-3.1-flash-lite');
});

test('loading: the cards are sketched and nothing can be sent until P3 answers', async () => {
  api(() => 'hang');
  const s2 = open('/preferences');
  expect($(s2, '[aria-busy="true"]')).not.toBeNull();
  expect(button(s2, '저장').disabled).toBe(true);
  expect(features(s2)).toEqual(['delete-api-key', 'set-api-key', 'youtube-logout']);
});

test('a failed load says so and loads again', async () => {
  let fail = true;
  api(() => (fail ? [500, {}] : [200, NONE]));
  const s2 = open('/preferences');
  // Under the field, which stays usable with its plain border: nothing typed is at fault.
  await vi.waitFor(() => expect(text($(s2, '[role="alert"]')!)).toBe("설정을 불러오지 못했습니다. 잠시 뒤에 다시 눌러 주세요 다시 불러오기"));
  expect($(s2, '#api-key')!.getAttribute('aria-invalid')).toBeNull();
  fail = false;
  button(s2, '다시 불러오기').click();
  await vi.waitFor(() => expect(text(s2)).toContain('개인키를 넣어야'));
});

test('saving: "확인하는 중" while Gemini checks; a refused or unchecked key keeps the value; a good one is masked', async () => {
  let release: (a: [number, unknown]) => void = () => {};
  api((c) => (c.method === 'GET' ? [200, NONE] : new Promise((r) => (release = r))));
  const s2 = open('/preferences');
  await vi.waitFor(() => expect(text(s2)).toContain('개인키를 넣어야'));
  const input = $(s2, '#api-key') as HTMLInputElement;
  expect(input.type).toBe('password');

  type(input, 'AIzaSy-refused-key-000000');
  button(s2, '저장').click();
  await vi.waitFor(() => expect(button(s2, '확인하는 중').disabled).toBe(true));
  release([401, { error: 'key_rejected', message: '…' }]);
  await vi.waitFor(() => expect(text($(s2, '#key-message')!)).toBe('쓸 수 없는 개인키입니다'));
  expect(input.value).toBe('AIzaSy-refused-key-000000');

  button(s2, '저장').click();
  await vi.waitFor(() => expect(button(s2, '확인하는 중').disabled).toBe(true));
  release([503, { error: 'gemini_unreachable', message: '…' }]);
  await vi.waitFor(() => expect(text($(s2, '#key-message')!)).toBe('지금은 확인할 수 없습니다. 잠시 뒤에 다시 눌러 주세요'));
  expect(input.value).toBe('AIzaSy-refused-key-000000');

  type(input, KEY);
  button(s2, '저장').click();
  await vi.waitFor(() => expect(button(s2, '확인하는 중').disabled).toBe(true));
  release([200, { key_set: true, key_masked: '••••WXYZ' }]);
  await vi.waitFor(() => expect(text(s2)).toContain('저장된 개인키 ••••WXYZ 끝 네 글자만 보입니다'));
  expect(input.value).toBe('');
  expect(text(s2)).not.toContain(KEY);
  expect(calls.filter((c) => c.method === 'PUT').map((c) => c.body.api_key)).toEqual(['AIzaSy-refused-key-000000', 'AIzaSy-refused-key-000000', KEY]);
});

test('deleting the key and signing out are each asked once more', async () => {
  api((c) => {
    if (c.method === 'GET') return [200, { ...NONE, key_set: true, key_masked: '••••WXYZ' }];
    if (c.path === '/settings/api-key') return [200, { key_set: false }];
    return [200, { cleared: true }];
  });
  const s2 = open('/preferences');
  await vi.waitFor(() => expect(text(s2)).toContain('••••WXYZ'));

  button(s2, '개인키 지우기').click();
  await vi.waitFor(() => expect(text(s2)).toContain('개인키를 지울까요?'));
  button(s2, '그만두기').click();
  await vi.waitFor(() => expect(text(s2)).not.toContain('개인키를 지울까요?'));
  expect(calls.filter((c) => c.method === 'DELETE')).toEqual([]);

  button(s2, '개인키 지우기').click();
  await vi.waitFor(() => button(s2, '지우기'));
  button(s2, '지우기').click();
  await vi.waitFor(() => expect(text(s2)).toContain('개인키를 이 기기에서 지웠습니다.'));
  expect(text(s2)).toContain('개인키를 넣어야 영상감지 버튼이 작업을 시작합니다');

  button(s2, '유튜브 로그아웃').click();
  await vi.waitFor(() => button(s2, '로그아웃'));
  button(s2, '로그아웃').click();
  await vi.waitFor(() => expect(text(s2)).toContain('유튜브에서 로그아웃했습니다.'));
  expect(calls.filter((c) => c.method === 'DELETE').map((c) => c.path)).toEqual(['/settings/api-key', '/browser/site-data']);
});
