import { expect, test, vi } from 'vitest';
import { $, api, button, calls, open, text } from './fake';

const summary = '<img src=x onerror=alert(1)>\n- Approved release notices.';
const update = (version = '0.1.7', platform = 'windows-x64') => ({
  version, summary,
  download_url: `https://github.com/LittleBitAI/video-translate-overlay-2/releases/download/v${version}/video-translate-overlay-${version}-${platform === 'android-arm64' ? 'arm64.apk' : 'windows-x64-setup.exe'}`,
});
const settings = { key_set: false, key_masked: null, translation_model: 'gemini' };
const answer = (value: unknown = update(), platform = 'windows-x64') => ({installed_version: '0.1.6', platform, update: value});

test.each(['android-arm64', 'windows-x64'])('launch notice displays text safely and invokes the %s download action', async platform => {
  const invoked: {command: string; args: any}[] = [];
  vi.stubGlobal('__TAURI_INTERNALS__', {invoke: async (command: string, args: any) => {
    invoked.push({command, args});
    if (command === 'check_updates') return answer(update('0.1.7', platform), platform);
    if (command === 'get_settings') return settings;
    return {opened: true};
  }});
  const root = open('/preferences');
  await vi.waitFor(() => expect(text(root)).toContain('새 버전 0.1.7'));
  expect(text(root)).toContain(summary.replace(/\s+/g, ' '));
  expect($(root, 'aside img')).toBeNull();
  const link = button(root, '업데이트 내려받기');
  expect(link.getAttribute('href')).toBe(update('0.1.7', platform).download_url);
  link.click();
  await vi.waitFor(() => expect(invoked.some(c => c.command === 'open_update' && c.args.version === '0.1.7')).toBe(true));
  await vi.waitFor(() => expect(button(root, '이 버전 알림 닫기').disabled).toBe(false));
  button(root, '이 버전 알림 닫기').click();
  await vi.waitFor(() => expect($(root, 'aside')).toBeNull());
  expect(invoked).toContainEqual({command: 'dismiss_update', args: {version: '0.1.7'}});
});

test('manual checks reveal dismissed releases and distinguish failure from no update', async () => {
  let result: any = answer();
  let status = 200;
  api(c => c.path === '/updates/check' ? [status, result] : [200, settings]);
  const root = open('/preferences');
  button(root, '업데이트 확인').click();
  await vi.waitFor(() => expect(text(root)).toContain('새 버전 0.1.7'));
  expect(calls.find(c => c.path === '/updates/check')?.body).toEqual({manual: true});
  result = answer(null);
  button(root, '업데이트 확인').click();
  await vi.waitFor(() => expect(text(root)).toContain('사용할 수 있는 새 업데이트가 없습니다'));
  status = 503;
  button(root, '업데이트 확인').click();
  await vi.waitFor(() => expect(text(root)).toContain('업데이트를 확인하지 못했습니다'));
  expect(text(root)).not.toContain('사용할 수 있는 새 업데이트가 없습니다');
});

test('repeated foreground events share a pending request and a queued manual check reveals the release', async () => {
  let release: (v: any) => void = () => {};
  let checks = 0;
  vi.stubGlobal('__TAURI_INTERNALS__', {invoke: (command: string) => {
    if (command === 'check_updates') {checks++; return new Promise(r => release = r);}
    return Promise.resolve(settings);
  }});
  const root = open('/preferences');
  await vi.waitFor(() => expect(checks).toBe(1));
  for (let i = 0; i < 10; i++) window.dispatchEvent(new Event('vto-foreground'));
  expect(checks).toBe(1);
  button(root, '업데이트 확인').click();
  release(answer(null));
  await vi.waitFor(() => expect(checks).toBe(2));
  release(answer());
  await vi.waitFor(() => expect(text(root)).toContain('새 버전 0.1.7'));
});

test('automatic failure and unsafe destination never become a usable notice', async () => {
  let response: any = null;
  vi.stubGlobal('__TAURI_INTERNALS__', {invoke: async (command: string) => {
    if (command !== 'check_updates') return settings;
    if (!response) throw {status: 503};
    return response;
  }});
  const root = open('/preferences');
  await new Promise(r => setTimeout(r, 40));
  expect($(root, 'aside')).toBeNull();
  response = answer({...update(), download_url: 'https://evil.test/app.exe'});
  window.dispatchEvent(new Event('vto-foreground'));
  await new Promise(r => setTimeout(r, 40));
  expect($(root, 'aside')).toBeNull();
});

test.each(['automatic', 'manual', 'queued manual', 'manual failure'])('a later dismissal wins over a pending %s check', async mode => {
  let count = 0;
  let release: (v: any) => void = () => {};
  vi.stubGlobal('__TAURI_INTERNALS__', {invoke: async (command: string) => {
    if (command === 'check_updates' && ++count % 2 === 0) return new Promise(r => release = r);
    if (command === 'check_updates') return answer();
    return settings;
  }});
  const root = open('/preferences');
  await vi.waitFor(() => expect(text(root)).toContain('새 버전 0.1.7'));
  for (let round = 1; round <= 2; round++) {
    if (mode.startsWith('manual')) button(root, '업데이트 확인').click();
    else window.dispatchEvent(new Event('vto-foreground'));
    await vi.waitFor(() => expect(count).toBe(round * 2));
    if (mode === 'queued manual') button(root, '업데이트 확인').click();
    button(root, '이 버전 알림 닫기').click();
    await vi.waitFor(() => expect($(root, 'aside')).toBeNull());
    release(mode === 'manual failure' ? null : answer());
    await new Promise(r => setTimeout(r, 40));
    expect($(root, 'aside')).toBeNull();
    expect(count).toBe(round * 2);
    button(root, '업데이트 확인').click();
    await vi.waitFor(() => expect(text(root)).toContain('새 버전 0.1.7'));
  }
});
