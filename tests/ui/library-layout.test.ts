// S3 `/library` layout: the wide rail and grid, the narrow accordion, video cards, edit mode,
// the add panel, and the shared 영상 / 보관함 / 설정 menu on all three screens.
import { expect, test, vi } from 'vitest';
import '../../src/ui/app.css';
import { page, userEvent } from 'vitest/browser';
import { $, $$, api, button, calls, open, text, type } from './fake';

const yt = (id: string, title: string, extra = {}) =>
  ({video_key:id, title, page_url:`https://www.youtube.com/watch?v=${id}`, duration_ms:0, saved:true, active:false, deleting:false, ...extra});
const many = Array.from({length:12}, (_, i) => yt(`video${String(i).padStart(6, '0')}`, `Lecture ${i}`, {duration_ms:600000}));
const other = {video_key:'a'.repeat(64), title:'Seminar on attention kernels with a deliberately long title that must wrap', page_url:'https://www.lectures.example.org/s.mp4', duration_ms:3723000, saved:false, active:false, deleting:false};
const library = {videos:[...many, other], playlists:[
  {id:'p', name:'Deep learning', entries:many.slice(0, 10).map(v => v.video_key)},
  {id:'q', name:'Systems', entries:[other.video_key, many[10].video_key]},
  {id:'e', name:'Later', entries:[]},
]};
const serve = () => api(c => c.path === '/library/data' && c.method === 'GET' ? [200, library] : c.path === '/browser' ? [200, {url:null, job:null}] : [200, {changed:true}]);
const rows = (root: ParentNode) => $$(root, 'ul[aria-label="재생목록"] li > div > button');
const row = (root: ParentNode, name: string) => rows(root).find(b => text(b).startsWith(`${name} 영상 `)) as HTMLButtonElement;
const cards = (root: ParentNode) => $$(root, 'li[data-video-key]');

test.each([[1280, 720, 4], [1009, 720, 3], [1920, 1080, 7], [2560, 1440, 10]])('at %i px the rail lists every playlist and a click shows its grid inside the viewport', async (width, height, columns) => {
  await page.viewport(width, height);
  serve();
  const root = open('/library');
  await vi.waitFor(() => expect(rows(root)).toHaveLength(4));
  expect(rows(root).map(b => text(b))).toEqual(['전체 영상 영상 13개', 'Deep learning 영상 10개', 'Systems 영상 2개', 'Later 영상 0개']);
  for (const b of rows(root)) expect(b.querySelector('.aspect-video')).not.toBeNull();
  expect(row(root, '전체 영상').getAttribute('aria-current')).toBe('true');
  const top = cards(root)[0].getBoundingClientRect().top;
  expect(cards(root).filter(card => card.getBoundingClientRect().top === top)).toHaveLength(columns);
  row(root, 'Systems').click();
  await vi.waitFor(() => expect(row(root, 'Systems').getAttribute('aria-current')).toBe('true'));
  expect(row(root, '전체 영상').hasAttribute('aria-current')).toBe(false);
  expect(getComputedStyle(row(root, 'Systems')).borderLeftColor).not.toBe(getComputedStyle(row(root, 'Later')).borderLeftColor);
  expect(cards(root).map(c => c.dataset.videoKey)).toEqual([other.video_key, many[10].video_key]);
  const first = cards(root)[0].getBoundingClientRect();
  expect(scrollY).toBe(0);
  expect(first.top).toBeGreaterThanOrEqual(0);
  expect(first.bottom).toBeLessThanOrEqual(height);
  expect(first.right).toBeLessThanOrEqual(width);
  expect(cards(root)[1].getBoundingClientRect().top).toBe(first.top);
  expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(width);
});

// Property: whatever the scroll position, activating a list leaves its first card on screen.
// Both lists hold the same videos, so nothing but the reveal can change the position.
test.each([[1280, 720, 0], [384, 854, 45]])('at %i px activating a list after scrolling the previous one shows its first card', async (width, height, bar) => {
  await page.viewport(width, height);
  const videos = Array.from({length:36}, (_, i) => yt(`scroll${String(i).padStart(5, '0')}`, `Lecture ${i}`));
  const same = videos.map(v => v.video_key);
  const twins = {videos, playlists:[{id:'a', name:'First', entries:same}, {id:'b', name:'Second', entries:same}]};
  api(c => c.path === '/library/data' && c.method === 'GET' ? [200, twins] : c.path === '/browser' ? [200, {url:null, job:null}] : [200, {changed:true}]);
  const root = open('/library');
  await vi.waitFor(() => expect(rows(root)).toHaveLength(3));
  row(root, 'First').click();
  await vi.waitFor(() => expect(cards(root)).toHaveLength(36));
  await vi.waitFor(() => expect(document.documentElement.scrollHeight).toBeGreaterThan(height + 300));
  scrollTo({top: document.documentElement.scrollHeight});
  await vi.waitFor(() => expect(scrollY).toBeGreaterThan(300));
  row(root, 'Second').click();
  await vi.waitFor(() => expect(row(root, 'Second').getAttribute(width < 768 ? 'aria-expanded' : 'aria-current')).toBe('true'));
  await vi.waitFor(() => {
    const first = cards(root)[0].getBoundingClientRect();
    expect(first.top).toBeGreaterThanOrEqual(0);
    expect(first.bottom).toBeLessThanOrEqual(height - bar);
  });
});

test('at 384 px a tapped row expands its videos directly under it, one row at a time, pinned while they scroll', async () => {
  await page.viewport(384, 854);
  serve();
  const root = open('/library');
  await vi.waitFor(() => expect(rows(root)).toHaveLength(4));
  expect(cards(root)).toHaveLength(0);
  expect(rows(root).map(b => b.getAttribute('aria-expanded'))).toEqual(['false', 'false', 'false', 'false']);
  row(root, 'Deep learning').click();
  await vi.waitFor(() => expect(row(root, 'Deep learning').getAttribute('aria-expanded')).toBe('true'));
  const head = row(root, 'Deep learning').parentElement!;
  const list = head.nextElementSibling!.firstElementChild!;
  expect(list.tagName).toBe('OL');
  expect(list.querySelector('li[data-video-key]')!.getAttribute('data-video-key')).toBe(many[0].video_key);
  expect(cards(root)).toHaveLength(10);
  scrollTo(0, head.getBoundingClientRect().top + scrollY + 600);
  await vi.waitFor(() => expect(Math.abs(head.getBoundingClientRect().top)).toBeLessThan(1));
  expect(cards(root)[5].getBoundingClientRect().top).toBeGreaterThan(head.getBoundingClientRect().bottom - 1);
  expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(384);
  scrollTo(0, 0);
  row(root, 'Systems').click();
  await vi.waitFor(() => expect(row(root, 'Systems').getAttribute('aria-expanded')).toBe('true'));
  expect(row(root, 'Deep learning').getAttribute('aria-expanded')).toBe('false');
  expect(cards(root).map(c => c.dataset.videoKey)).toEqual([other.video_key, many[10].video_key]);
  expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(384);
  row(root, 'Systems').click();
  await vi.waitFor(() => expect(row(root, 'Systems').getAttribute('aria-expanded')).toBe('false'));
  expect(cards(root)).toHaveLength(0);
});

test('cards show a YouTube thumbnail or a host placeholder, duration, clamped title, site and status, and open like 영상 열기', async () => {
  await page.viewport(1280, 720);
  const videos = [yt('abcDEF12345', 'Saved lecture'), {...other, active:true}, yt('zzzDEF12345', 'Cleanup', {deleting:true, saved:false})];
  let opened = '';
  api(c => {
    if (c.path === '/library/data') return [200, {videos, playlists:[{id:'p', name:'Topic', entries:[other.video_key, 'abcDEF12345']}, {id:'e', name:'Empty', entries:[]}]}];
    if (c.path === '/browser/open') { opened = c.body.url; return [200, {opened_url:c.body.url}]; }
    return [200, {url:opened || null, job:null}];
  });
  const root = open('/library');
  await vi.waitFor(() => expect(cards(root)).toHaveLength(3));
  const [saved, active, deleting] = cards(root);
  expect(saved.querySelector('[data-src]')!.getAttribute('data-src')).toBe('https://i.ytimg.com/vi/abcDEF12345/mqdefault.jpg');
  expect(text(saved)).toContain('youtube.com');
  expect(text(saved)).toContain('저장 자막 있음');
  expect(text(saved)).not.toMatch(/\d:\d\d/);
  expect(active.querySelector('img')).toBeNull();
  expect(text(active.querySelector('[data-placeholder]')!)).toBe('lectures.example.org');
  expect(active.querySelector('[data-placeholder] svg')).not.toBeNull();
  expect(text(active)).toContain('1:02:03');
  expect(text(active)).toContain('처리 중');
  expect(text(deleting)).toContain('삭제 미완료');
  expect(button(deleting, '삭제 다시 시도')).toBeTruthy();
  const title = active.querySelector('[data-title]') as HTMLElement;
  expect(getComputedStyle(title).webkitLineClamp).toBe('2');
  expect(title.getBoundingClientRect().height).toBeLessThanOrEqual(2 * 16 * 1.5 + 1);
  // A real load of this unknown ID fails or returns YouTube's 120 px stand-in; force the failure either way.
  saved.querySelector('img')?.dispatchEvent(new Event('error'));
  await vi.waitFor(() => expect(text(saved.querySelector('[data-placeholder]')!)).toBe('youtube.com'));
  expect(row(root, 'Topic').querySelector('img')).toBeNull();
  expect(row(root, 'Topic').querySelector('[data-placeholder]')).not.toBeNull();
  row(root, 'Empty').click();
  await vi.waitFor(() => expect(text(root)).toContain('이 재생목록은 비어 있습니다'));
  expect(row(root, 'Empty').querySelector('[data-placeholder]')).not.toBeNull();
  row(root, '전체 영상').click();
  await vi.waitFor(() => expect(cards(root)).toHaveLength(3));
  (cards(root)[0].querySelector('button[aria-label="Saved lecture 영상 열기"]') as HTMLButtonElement).click();
  await vi.waitFor(() => expect(calls.find(c => c.path === '/browser/open')?.body).toEqual({url:'https://www.youtube.com/watch?v=abcDEF12345'}));
  await vi.waitFor(() => expect($(root, '#address')).not.toBeNull());
  expect(calls.some(c => c.path === '/jobs')).toBe(false);
});

test('the default view has only search, 추가 and 편집; edit mode holds every management control and reorders by buttons, keyboard and drag', async () => {
  await page.viewport(1280, 720);
  serve();
  const root = open('/library');
  await vi.waitFor(() => expect(rows(root)).toHaveLength(4));
  row(root, 'Deep learning').click();
  await vi.waitFor(() => expect(cards(root)).toHaveLength(10));
  const pane = $(root, 'section[aria-label="영상 목록"]')!;
  expect($$(root, 'form')).toHaveLength(0);
  expect($$(root, 'main button').filter(b => !b.closest('li')).map(b => text(b))).toEqual(['추가', '편집']);
  expect($(pane, 'input[type="search"]')).not.toBeNull();
  for (const label of ['위로', '아래로', '목록에서 빼기', '영상 삭제', '재생목록 삭제', '이름 바꾸기']) expect(text(root)).not.toContain(label);
  expect($(root, '[title="끌어서 옮기기"]')).toBeNull();
  button(root, '편집').click();
  await vi.waitFor(() => expect(button(root, '완료')).toBeTruthy());
  for (const label of ['목록에서 빼기', '영상 삭제', '재생목록 삭제', '이름 바꾸기']) expect(button(root, label)).toBeTruthy();
  expect(($(root, 'form input:not([type])') as HTMLInputElement).value).toBe('Deep learning');
  expect(text(cards(root)[0])).toContain(many[0].page_url);
  const down = $(root, `[aria-label="${many[0].title} 영상 아래로"]`) as HTMLButtonElement;
  expect(text(down)).toBe('아래로');
  expect(text($(root, '[aria-label="Systems 재생목록 위로"]')!)).toBe('위로');
  expect($$(root, '[title="끌어서 옮기기"]')).toHaveLength(13);
  down.focus();
  await userEvent.keyboard('{Enter}');
  await vi.waitFor(() => expect(calls.filter(c => c.body?.action === 'move_entry')).toHaveLength(1));
  expect(calls.find(c => c.body?.action === 'move_entry')!.body).toMatchObject({playlist_id:'p', video_key:many[0].video_key, delta:1});
  await vi.waitFor(() => expect(down.disabled).toBe(false));
  const handle = cards(root)[0].querySelector('[title="끌어서 옮기기"]')!;
  const target = cards(root)[3].getBoundingClientRect();
  const at = (r: DOMRect) => ({clientX:r.left + r.width / 2, clientY:r.top + r.height / 2, bubbles:true, pointerId:1});
  handle.dispatchEvent(new PointerEvent('pointerdown', at(handle.getBoundingClientRect())));
  dispatchEvent(new PointerEvent('pointermove', at(target)));
  await vi.waitFor(() => expect(cards(root)[3].className).toContain('ring-2'));
  dispatchEvent(new PointerEvent('pointerup', at(target)));
  await vi.waitFor(() => expect(calls.filter(c => c.body?.action === 'move_entry')).toHaveLength(2));
  expect(calls.filter(c => c.body?.action === 'move_entry')[1].body).toMatchObject({video_key:many[0].video_key, delta:3});
  await vi.waitFor(() => expect(down.disabled).toBe(false));
  const rail = $$(root, 'ul[aria-label="재생목록"] li[data-drag="playlist"]');
  rail[2].querySelector('[title="끌어서 옮기기"]')!.dispatchEvent(new PointerEvent('pointerdown', at(rail[2].getBoundingClientRect())));
  dispatchEvent(new PointerEvent('pointermove', at(rail[0].getBoundingClientRect())));
  dispatchEvent(new PointerEvent('pointerup', at(rail[0].getBoundingClientRect())));
  await vi.waitFor(() => expect(calls.find(c => c.body?.action === 'move_playlist')?.body).toMatchObject({playlist_id:'e', delta:-2}));
  await vi.waitFor(() => expect(button(root, '완료').disabled).toBe(false));
  button(root, '완료').click();
  await vi.waitFor(() => expect(text(root)).not.toContain('목록에서 빼기'));
  expect(calls.some(c => c.path === '/jobs')).toBe(false);
});

test('추가 offers saved videos with thumbnails, a link with an optional name and the cost note, and + 새 재생목록', async () => {
  await page.viewport(384, 854);
  let made = false;
  api(c => {
    if (c.method === 'GET') return [200, {videos:[many[0], many[1], other], playlists:[
      {id:'p', name:'Topic', entries:[many[0].video_key]}, ...(made ? [{id:'new', name:'Fresh', entries:[]}] : [])]}];
    if (c.body.action === 'create_playlist') made = true;
    return [200, {changed:true}];
  });
  const root = open('/library');
  await vi.waitFor(() => expect(rows(root)).toHaveLength(2));
  row(root, 'Topic').click();
  button(root, '추가').click();
  const panel = await vi.waitFor(() => { const p = $(root, 'section[aria-label="추가"]'); expect(p).not.toBeNull(); return p!; });
  expect(row(root, 'Topic').parentElement!.nextElementSibling!.firstElementChild).toBe(panel);
  const pick = $$(panel, 'button').find(b => text(b) === `${many[1].title} 추가`)!;
  expect(pick.querySelector('[data-src]')!.getAttribute('data-src')).toContain(many[1].video_key);
  expect(text(panel)).not.toContain(other.title);
  expect(text(panel)).toContain('API 비용');
  pick.click();
  await vi.waitFor(() => expect(calls.find(c => c.body?.action === 'add_entry')?.body).toEqual({action:'add_entry', playlist_id:'p', page_url:many[1].page_url, title:many[1].title}));
  await vi.waitFor(() => expect(calls.filter(c => c.method === 'GET')).toHaveLength(2));
  type($(panel, 'input[type="url"]'), 'https://lectures.example.org/new.mp4');
  type($$(panel, 'input:not([type])')[0], 'Named link');
  await vi.waitFor(() => expect(button(panel, '링크 추가').disabled).toBe(false));
  button(panel, '링크 추가').click();
  await vi.waitFor(() => expect(calls.filter(c => c.body?.action === 'add_entry')[1]?.body).toEqual({action:'add_entry', playlist_id:'p', page_url:'https://lectures.example.org/new.mp4', title:'Named link'}));
  await vi.waitFor(() => expect(calls.filter(c => c.method === 'GET')).toHaveLength(3));
  type($$(panel, 'input:not([type])')[1], 'Fresh');
  await vi.waitFor(() => expect(button(panel, '+ 새 재생목록').disabled).toBe(false));
  button(panel, '+ 새 재생목록').click();
  await vi.waitFor(() => expect(row(root, 'Fresh')?.getAttribute('aria-expanded')).toBe('true'));
  expect(calls.find(c => c.body?.action === 'create_playlist')?.body).toEqual({action:'create_playlist', name:'Fresh'});
  expect(calls.some(c => c.path === '/jobs')).toBe(false);
  expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(384);
});

const everyScreen = (url: string | null = null) => api(c => {
  if (c.path === '/library/data') return [200, {videos:[], playlists:[]}];
  if (c.path === '/settings') return [200, {key_set:false, key_masked:null, translation_model:'gemini-3.1-flash-lite'}];
  if (c.path === '/account') return [200, {generation:0, state:'unsupported', email:null, error:null, sync_ready:false}];
  return [200, {url, job:null}];
});

test.each([[1280, 720], [1920, 1080], [2560, 1440], [768, 720], [384, 854]])('at %i px the shared menu sits in the same place on every screen and marks the current one', async (width, height) => {
  await page.viewport(width, height);
  everyScreen();
  const root = open('/');
  const seen: Record<string, object[]> = {};
  for (const [path, label] of [['/', '영상'], ['/library', '보관함'], ['/preferences', '설정']] as const) {
    if (path !== '/') button(root, label).click();
    await vi.waitFor(() => expect($(root, 'nav[aria-label="주 메뉴"] [aria-current="page"]')?.textContent).toBe(label));
    const nav = $(root, 'nav[aria-label="주 메뉴"]')!;
    const links = $$(nav, 'a');
    expect(links.map(a => [text(a), a.getAttribute('href')])).toEqual([['영상', '/'], ['보관함', '/library'], ['설정', '/preferences']]);
    const current = links.find(a => a.getAttribute('aria-current') === 'page')!;
    const quiet = links.find(a => a !== current)!;
    expect(getComputedStyle(current).fontWeight).not.toBe(getComputedStyle(quiet).fontWeight);
    for (const a of links) expect(a.getBoundingClientRect().height).toBeGreaterThanOrEqual(44);
    seen[path] = links.map(a => a.getBoundingClientRect().toJSON());
    expect(!!$(root, '#address')).toBe(path === '/');
    if (width < 768) {
      expect(getComputedStyle(nav).position).toBe('fixed');
      expect(nav.getBoundingClientRect().bottom).toBe(height);
      expect(nav.className).toContain('pb-[env(safe-area-inset-bottom)]');
    } else {
      expect(nav.getBoundingClientRect().top).toBe(0);
      expect(nav.getBoundingClientRect().left).toBe(16);
      const main = $(root, 'main')!;
      expect(main.getBoundingClientRect().left).toBe(0);
      expect(getComputedStyle(main).paddingLeft).toBe('16px');
      if (path === '/') {
        const stage = $(root, '.video-stage')!.getBoundingClientRect();
        expect(stage.left).toBe(16);
        expect(stage.width).toBeGreaterThan(width * 0.8);
        expect(stage.bottom).toBeLessThanOrEqual(height);
        expect(stage.width / stage.height).toBeCloseTo(16 / 9, 1);
      }
      if (path === '/preferences') {
        const account = $(root, 'section[aria-label="Google 앱 계정"]')!.getBoundingClientRect();
        const key = $(root, '#api-key')!.closest('section')!.getBoundingClientRect();
        expect(key.left).toBe(16);
        expect($(root, '#api-key')!.closest('form')!.getBoundingClientRect().width).toBeLessThanOrEqual(672);
        if (width >= 1024) {
          expect(account.top).toBe(key.top);
          expect(account.left).toBeGreaterThan(key.right);
          for (const section of $$(root, '.settings-section')) {
            expect(getComputedStyle(section).backgroundColor).toBe('rgba(0, 0, 0, 0)');
            expect(getComputedStyle(section).borderRadius).toBe('0px');
          }
        } else {
          expect(account.top).toBeGreaterThan(key.bottom);
        }
      }
    }
    if (path === '/preferences') {
      expect($$(root, 'h2').map(text)).toEqual(['Gemini 개인키', 'Google 앱 계정', '유튜브 로그인', '앱 업데이트']);
      const footer = $(root, 'main footer')!;
      expect(footer.getBoundingClientRect().top).toBeGreaterThanOrEqual(Math.max(...$$(root, '.settings-section').map(section => section.getBoundingClientRect().bottom)));
      expect(text(footer)).toContain('개인정보처리방침');
    }
    expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(width);
  }
  expect(seen['/library']).toEqual(seen['/']);
  expect(seen['/preferences']).toEqual(seen['/']);
  expect(text(root)).not.toContain('← 영상으로');
  button(root, '보관함').click();
  await vi.waitFor(() => expect(text(root)).toContain('아직 저장한 영상이 없습니다'));
  expect($$(root, 'a').filter(a => text(a) === '영상 열기')).toHaveLength(0);
});

test.each([[1280, 720], [2560, 1440], [384, 854]])('at %i px an opened video and its overlay share a visible 16:9 stage', async (width, height) => {
  await page.viewport(width, height);
  everyScreen('https://lectures.example.org/layout.mp4');
  const root = open('/');
  await vi.waitFor(() => expect($(root, 'video')).not.toBeNull());
  const frame = $(root, 'video')!.getBoundingClientRect();
  const stage = $(root, '.video-stage')!.getBoundingClientRect();
  expect(frame.width / frame.height).toBeCloseTo(16 / 9, 1);
  expect(frame.width).toBeCloseTo(stage.width, 1);
  expect(frame.bottom).toBeLessThanOrEqual(height);
  expect(frame.right).toBeLessThanOrEqual(width);
  const overlay = $(root, '.pointer-events-none')!.getBoundingClientRect();
  expect(overlay.width).toBeCloseTo(frame.width, 1);
  expect(overlay.height).toBeCloseTo(frame.height, 1);
  expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(width);
});
