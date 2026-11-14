import { expect, test, vi } from 'vitest';
import '../../src/ui/app.css';
import { page } from 'vitest/browser';
import { $, button, open, text } from './fake';

type Account = { generation: number; state: string; email: string | null; error: string | null; sync_ready: boolean };
const signedOut = (generation = 7): Account => ({ generation, state: 'signed_out', email: null, error: null, sync_ready: false });
const settings = { key_set: true, key_masked: '••••WXYZ', translation_model: 'gemini-3.1-flash-lite' };

function native(handler: (command: string, args: any) => unknown | Promise<unknown>) {
  const calls: { command: string; args: any }[] = [];
  vi.stubGlobal('__TAURI_INTERNALS__', { invoke: async (command: string, args: any) => {
    calls.push({ command, args });
    if (command === 'get_settings') return settings;
    if (command === 'check_updates') return { update: null, platform: 'windows' };
    if (command === 'get_browser') return { url: null, job: null };
    return handler(command, args);
  } });
  return calls;
}

test('native sign-in uses its captured account generation and observes immediate completion', async () => {
  let account = signedOut();
  const calls = native((command, args) => {
    if (command === 'get_google_account') return account;
    expect(command).toBe('google_sign_in');
    expect(args).toEqual({ generation: 7 });
    account = { ...signedOut(8), state: 'signed_in', email: 'one@example.org' };
    return { ...signedOut(8), state: 'signing_in' };
  });
  const root = open('/preferences');
  await vi.waitFor(() => expect(button(root, 'Google 계정으로 로그인').disabled).toBe(false));
  button(root, 'Google 계정으로 로그인').click();
  await vi.waitFor(() => expect(text(root)).toContain('연결한 Google 계정: one@example.org'));
  expect(text(root)).toContain('기기간 동기화는 아직 연결되지 않았습니다');
  expect(calls.filter(c => c.command === 'google_sign_in')).toHaveLength(1);
  expect(calls.some(c => c.command === 'browser_open')).toBe(false);
});

test('Google sign-out requires confirmation and preserves Gemini and YouTube controls', async () => {
  let account = { ...signedOut(9), state: 'signed_in', email: 'one@example.org' };
  const calls = native((command, args) => {
    if (command === 'get_google_account') return account;
    expect(command).toBe('google_sign_out');
    expect(args).toEqual({ generation: 9 });
    return account = signedOut(10);
  });
  const root = open('/preferences');
  await vi.waitFor(() => expect(text(root)).toContain('one@example.org'));
  button(root, 'Google 계정 로그아웃').click();
  await vi.waitFor(() => expect($(root, '[aria-label="Google 계정 로그아웃 확인"]')).not.toBeNull());
  expect(calls.some(c => c.command === 'google_sign_out')).toBe(false);
  button(root, '그만두기').click();
  await vi.waitFor(() => expect($(root, '[aria-label="Google 계정 로그아웃 확인"]')).toBeNull());
  button(root, 'Google 계정 로그아웃').click();
  await vi.waitFor(() => expect($(root, '[aria-label="Google 계정 로그아웃 확인"]')).not.toBeNull());
  button(root, 'Google 로그아웃 확인').click();
  await vi.waitFor(() => expect(text(root)).toContain('연결한 Google 계정이 없습니다'));
  expect(text(root)).toContain('••••WXYZ');
  expect(calls.some(c => ['clear_site_data', 'delete_api_key'].includes(c.command))).toBe(false);
});

test('cancel and a newer sign-in retire an older pending status response without blocking polling', async () => {
  let account = signedOut();
  let pending = false;
  let release: (value: Account) => void = () => {};
  native(command => {
    if (command === 'google_sign_in') {
      account = { ...signedOut(account.generation + 1), state: 'signing_in' };
      return account;
    }
    if (command === 'google_sign_out') return account = signedOut(account.generation + 1);
    if (account.generation === 8) {
      pending = true;
      return new Promise<Account>(resolve => release = resolve);
    }
    if (account.generation === 10) return { ...account, state: 'signed_in', email: 'new@example.org' };
    return account;
  });
  const root = open('/preferences');
  await vi.waitFor(() => expect(button(root, 'Google 계정으로 로그인')).toBeTruthy());
  button(root, 'Google 계정으로 로그인').click();
  await vi.waitFor(() => expect(pending).toBe(true));
  button(root, 'Google 로그인 취소').click();
  await vi.waitFor(() => expect(text(root)).toContain('연결한 Google 계정이 없습니다'));
  button(root, 'Google 계정으로 로그인').click();
  await vi.waitFor(() => expect(text(root)).toContain('new@example.org'));
  release({ ...signedOut(8), state: 'signed_in', email: 'old@example.org' });
  await new Promise(resolve => setTimeout(resolve, 30));
  expect(text(root)).toContain('new@example.org');
  expect(text(root)).not.toContain('old@example.org');
});

test('an unsolicited account generation requires explicit reload before adopting the other account', async () => {
  let account = signedOut();
  native(command => {
    if (command === 'google_sign_in') {
      account = { ...signedOut(20), state: 'signed_in', email: 'other@example.org' };
      return { ...signedOut(8), state: 'signing_in' };
    }
    return account;
  });
  const root = open('/preferences');
  await vi.waitFor(() => expect(button(root, 'Google 계정으로 로그인')).toBeTruthy());
  button(root, 'Google 계정으로 로그인').click();
  await vi.waitFor(() => expect(text(root)).toContain('다른 화면에서 계정 상태가 바뀌었습니다'));
  expect(text(root)).not.toContain('other@example.org');
  button(root, '계정 상태 다시 불러오기').click();
  await vi.waitFor(() => expect(text(root)).toContain('other@example.org'));
});

test('expired authentication and failed re-queries show recovery without claiming success', async () => {
  let fail = false;
  native(command => {
    if (command === 'google_sign_in') { fail = true; return { ...signedOut(8), state: 'signing_in' }; }
    if (fail) throw { status: 503, message: 'unavailable' };
    return { ...signedOut(), state: 'expired', error: 'Google authentication expired' };
  });
  const root = open('/preferences');
  await vi.waitFor(() => expect(text(root)).toContain('Google 로그인이 만료됐습니다'));
  button(root, 'Google 계정으로 로그인').click();
  await vi.waitFor(() => expect(text(root)).toContain('Google 계정 상태를 확인하지 못했습니다'));
  button(root, '계정 상태 다시 불러오기').click();
  await vi.waitFor(() => expect(text(root)).toContain('Google 계정 상태를 불러오지 못했습니다'));
  expect(text(root)).not.toContain('연결한 Google 계정:');
});

test('an initial account response cannot publish into a replacement settings screen', async () => {
  let reads = 0;
  let release: (value: Account) => void = () => {};
  native(() => ++reads === 1 ? new Promise<Account>(resolve => release = resolve) : signedOut());
  const root = open('/preferences');
  await vi.waitFor(() => expect(reads).toBe(1));
  button(root, '← 영상으로').click();
  await vi.waitFor(() => expect(text(root)).toContain('영상 주소를 넣으면'));
  button(root, '설정').click();
  await vi.waitFor(() => expect(text(root)).toContain('연결한 Google 계정이 없습니다'));
  release({ ...signedOut(), state: 'signed_in', email: 'retired@example.org' });
  await new Promise(resolve => setTimeout(resolve, 30));
  expect(text(root)).not.toContain('retired@example.org');
});

test('publisher registration failure names the app configuration and keeps account credentials private', async () => {
  native(() => ({ ...signedOut(), error: 'Google Desktop registration is missing or invalid' }));
  const root = open('/preferences');
  await vi.waitFor(() => expect(text(root)).toContain('앱 제공자가 Google 로그인 등록 정보를 수정해야 합니다'));
  expect(text(root)).not.toContain('계정과 네트워크를 확인');
  expect(text(root)).not.toContain('client_secret');
  expect(text(root)).not.toContain('Google Desktop registration');
});

test.each([384, 400, 1280])('Google account controls and long addresses fit a %s pixel screen', async width => {
  await page.viewport(width, 854);
  native(() => ({ ...signedOut(), state: 'signed_in', email: `${'account'.repeat(25)}@example.org` }));
  const root = open('/preferences');
  await vi.waitFor(() => expect(button(root, 'Google 계정 로그아웃')).toBeTruthy());
  const card = $(root, '[aria-label="Google 앱 계정"]')!;
  const rect = card.getBoundingClientRect();
  expect(rect.left).toBeGreaterThanOrEqual(0);
  expect(rect.right).toBeLessThanOrEqual(width);
  expect(card.scrollWidth).toBeLessThanOrEqual(card.clientWidth);
  const control = button(card, 'Google 계정 로그아웃');
  expect(control.getBoundingClientRect().height).toBeGreaterThanOrEqual(44);
  expect(getComputedStyle(control).fontSize).toBe('16px');
  const title = card.querySelector('h2')!;
  expect(getComputedStyle(title).fontSize).toBe(getComputedStyle(root.querySelector('section h2')!).fontSize);
  button(card, 'Google 계정 로그아웃').click();
  await vi.waitFor(() => expect(card.querySelector('[role="alertdialog"]')).not.toBeNull());
  expect(card.scrollWidth).toBeLessThanOrEqual(card.clientWidth);
});
