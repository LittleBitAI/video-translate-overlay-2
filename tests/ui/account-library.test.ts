import { expect, test, vi } from 'vitest';
import { request } from '../../src/ui/api';
import '../../src/ui/app.css';
import { page } from 'vitest/browser';
import { $, api, button, calls, open, text } from './fake';

test.each([384, 1280])('retained subtitle versions at %i px can be selected without starting translation', async width => {
  await page.viewport(width, 854);
  let selected = 'new';
  const video = {video_key:'saved', title:'Retained lecture', page_url:'https://lectures.example.org/a', saved:true, active:false, deleting:false};
  api(c => {
    if (c.method === 'GET') return [200, {account_generation:9, videos:[{...video, versions:['new','old'].map(id => ({id,selected:id===selected}))}], playlists:[]}];
    expect(c.path).toBe('/library/data');
    expect(c.body).toEqual({action:'select_translation',video_key:'saved',translation_id:'old',account_generation:9});
    selected = 'old'; return [200, {changed:true}];
  });
  const root = open('/library');
  await vi.waitFor(() => expect(text(root)).toContain('저장된 자막 2개'));
  const select = $(root, 'select[aria-label="Retained lecture 저장 자막 선택"]') as HTMLSelectElement;
  expect(select.value).toBe('new');
  expect(select.getBoundingClientRect().height).toBeGreaterThanOrEqual(44);
  expect(select.getBoundingClientRect().right).toBeLessThanOrEqual(width);
  select.value='old'; select.dispatchEvent(new Event('change', {bubbles:true}));
  await vi.waitFor(() => expect(select.value).toBe('old'));
  await vi.waitFor(() => expect(text(root)).toContain('저장본 2 · 사용 중'));
  expect(calls.filter(c => c.method==='POST')).toHaveLength(1);
  expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(width);
});

test.each([384, 1280])('legacy import at %i px names the account copy and sends consent only after confirmation', async width => {
  await page.viewport(width, 854);
  let imported = false;
  api(c => {
    if (c.method === 'GET') return [200, { videos: [], playlists: [], legacy_available: !imported, account_generation: 17, account_email:'one@example.org' }];
    expect(c.body).toEqual({ action: 'import_legacy', confirmed: true, account_generation: 17 });
    imported = true;
    return [200, { changed: true }];
  });
  const root = open('/library');
  await vi.waitFor(() => expect(text(root)).toContain('이 기기의 기존 보관함'));
  const card = $(root, '[aria-label="기존 보관함 가져오기"]')!;
  await vi.waitFor(() => expect(button(card, '기존 보관함 가져오기').getBoundingClientRect().height).toBeGreaterThanOrEqual(44));
  expect(card.getBoundingClientRect().right).toBeLessThanOrEqual(width);
  expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(width);
  button(root, '기존 보관함 가져오기').click();
  await vi.waitFor(() => expect(text(root)).toContain('같은 계정의 연결된 기기에도 전달됩니다'));
  expect(text(card)).toContain('one@example.org으로 복사할까요?');
  button(root, '취소').click();
  await vi.waitFor(() => expect(button(root, '기존 보관함 가져오기')).toBeTruthy());
  expect(calls.filter(c => c.method === 'POST')).toHaveLength(0);
  button(root, '기존 보관함 가져오기').click();
  await vi.waitFor(() => expect(button(root, '현재 계정에 복사')).toBeTruthy());
  button(root, '현재 계정에 복사').click();
  await vi.waitFor(() => expect($(root, '[aria-label="기존 보관함 가져오기"]')).toBeNull());
  expect(calls.filter(c => c.method === 'POST')).toHaveLength(1);
  expect(calls.some(c => c.path === '/jobs')).toBe(false);
});

test('account changes discard a deletion confirmation and late library rows before showing the new account', async () => {
  let owner = 1;
  let late: ((answer: any) => void) | undefined;
  let reads = 0;
  const video = { video_key:'same', title:'Previous account lecture', page_url:'https://lectures.example.org/a', saved:true, active:false, deleting:false };
  api(c => {
    expect(c.method).toBe('GET');
    reads++;
    if (reads === 2) return new Promise(resolve => { late = resolve; });
    return [200, { account_generation:owner, videos:owner === 1 ? [video] : [], playlists:[] }];
  });
  const root = open('/library');
  await vi.waitFor(() => expect(text(root)).toContain(video.title));
  button(root, '영상 삭제').click();
  await vi.waitFor(() => expect($(root, '[role="alertdialog"]')).not.toBeNull());
  window.dispatchEvent(new CustomEvent('vto-video-deleted', {detail:'unrelated'}));
  await vi.waitFor(() => expect(late).toBeTruthy());
  owner = 2;
  window.dispatchEvent(new Event('vto-account-changed'));
  await vi.waitFor(() => expect(reads).toBe(3));
  late!([200, { account_generation:1, videos:[video], playlists:[] }]);
  await vi.waitFor(() => expect(text(root)).not.toContain(video.title));
  expect($(root, '[role="alertdialog"]')).toBeNull();
  expect(calls.some(c => c.method === 'POST')).toBe(false);
});

test('native library transport captures its generation and rejects a reply completed after account retirement', async () => {
  window.dispatchEvent(new Event('vto-account-changed'));
  const seen: { command: string; args: any }[] = [];
  let answer: ((body: any) => void) | undefined;
  vi.stubGlobal('__TAURI_INTERNALS__', {invoke: async (command: string, args: any) => {
    seen.push({command, args});
    if (command === 'get_google_account') return {generation:31};
    return new Promise(resolve => { answer = resolve; });
  }});
  const pending = request('GET', '/library/data');
  await vi.waitFor(() => expect(answer).toBeTruthy());
  expect(seen[1]).toEqual({command:'get_library', args:{account_generation:31}});
  window.dispatchEvent(new Event('vto-account-changed'));
  answer!({account_generation:31, videos:[{title:'Retired private video'}], playlists:[]});
  const result = await pending;
  expect(result.status).toBe(409);
  expect(result.body.error).toBe('account_changed');
  expect(JSON.stringify(result)).not.toContain('Retired private video');
});
