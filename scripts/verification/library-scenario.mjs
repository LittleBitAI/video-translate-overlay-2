import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import path from 'node:path';
import {fixture} from './fixture.mjs';

export const assertions = [
  {id: 'saved-reuse', expected: 'Saved subtitles retain their times and cost without a translation request'},
  {id: 'restart', expected: 'Playlist names, order and memberships survive a process restart'},
  {id: 'cancel-delete', expected: 'Cancelling named permanent deletion preserves the saved translation'},
  {id: 'delete', expected: 'Confirmed deletion removes the video from every playlist and preserves unrelated data'},
  {id: 'manual-link', expected: 'Opening an untranslated playlist entry leaves translation unstarted'},
];

export async function call(page, method, route, body = {}) {
  return page.evaluate(async ({method, route, body}) => {
    if (window.__TAURI_INTERNALS__) {
      const subs = /^\/videos\/([^/]+)\/subtitles$/.exec(route);
      const command = subs ? 'get_subtitles' : ({'GET /library/data': 'get_library', 'POST /library/data': 'library_action',
        'GET /jobs': 'get_job', 'POST /browser/open': 'browser_open'})[`${method} ${route}`];
      const args = subs ? {video_key: subs[1]} : method === 'POST' && route === '/library/data' ? {action: body} : body;
      try {return {status: 200, body: await window.__TAURI_INTERNALS__.invoke(command, args)};}
      catch (error) {return {status: error.status ?? 500, body: error};}
    }
    const suffix = method === 'GET' && Object.keys(body).length ? `?${new URLSearchParams(body)}` : '';
    const response = await fetch(route + suffix, {method,
      ...(method !== 'GET' ? {headers: {'Content-Type': 'application/json'}, body: JSON.stringify(body)} : {})});
    return {status: response.status, body: await response.json()};
  }, {method, route, body});
}

export default async function scenario(session) {
  const media = await fixture(session.root);
  try {
    const directory = path.join(session.artifact, 'data');
    execFileSync(process.env.VTO_VERIFICATION_PYTHON ?? 'python', [path.join(session.root, 'scripts/verification/seed.py'), directory,
      session.platform === 'android' ? '10.0.2.2' : '127.0.0.1', '8137'], {windowsHide: true});
    await session.start(directory);
    let page = session.page;
    const click = (name, scope = page) => scope.getByRole('button', {name, exact: true}).click();
    const library = async () => {
      await page.getByRole('link', {name: '보관함', exact: true}).click();
      await page.getByLabel('제목 또는 주소 검색').waitFor();
    };
    const list = async () => {const r = await call(page, 'GET', '/library/data'); assert.equal(r.status, 200); return r.body;};
    const row = title => page.locator('li[data-video-key]').filter({has: page.getByRole('heading', {name: title, exact: true})});
    await library();
    const before = await list(); assert.equal(before.videos.length, 2); assert.ok(before.videos.every(video => video.saved));
    const saved = before.videos.find(video => video.title === 'Saved attention');
    const subsBefore = await call(page, 'GET', `/videos/${saved.video_key}/subtitles`);
    const costBefore = await call(page, 'GET', '/jobs', {page_url: saved.page_url});
    assert.equal(costBefore.body.cost_krw, 12.5);
    for (const name of ['Deep learning', 'Systems']) {
      await page.getByLabel('새 재생목록 이름').fill(name); await click('재생목록 만들기');
      await click(`${name} (0)`);
      await page.getByLabel('저장된 영상', {exact: true}).selectOption({label: 'Saved attention'}); await click('저장 영상 추가');
      await session.until(async () => (await list()).playlists.find(p => p.name === name)?.entries.length === 1);
    }
    await page.getByLabel('저장된 영상', {exact: true}).selectOption({label: 'Unrelated systems'}); await click('저장 영상 추가');
    await session.until(async () => (await list()).playlists[1].entries.length === 2);
    await page.getByLabel('영상 링크', {exact: true}).fill(saved.page_url.replace('saved.mp4', 'untranslated.mp4'));
    await page.getByLabel('영상 이름 (선택)').fill('Untranslated lecture'); await click('링크 추가');
    await click('Systems (3)');
    await page.getByLabel('재생목록 이름', {exact: true}).fill('Systems topic'); await click('이름 바꾸기');
    await click('Systems topic 재생목록 위로');
    await session.until(async () => (await list()).playlists[0].name === 'Systems topic');
    await click('Untranslated lecture 영상 위로');
    await click('목록에서 빼기', row('Unrelated systems'));
    await session.until(async () => (await list()).playlists[0].entries.length === 2);
    const organized = await list();
    await click('영상 열기', row('Saved attention'));
    const videoPage = session.platform === 'windows'
      ? await session.until(() => session.context.pages().find(p => p.url().includes('/saved.mp4'))) : page;
    await videoPage.locator('video').waitFor();
    await videoPage.evaluate(() => {const video = document.querySelector('video'); video.currentTime = 1; video.dispatchEvent(new Event('timeupdate'));});
    const cue = videoPage.locator(session.platform === 'browser' ? '[data-feature="watch-subtitles"]' : '[data-vto="subtitle"]');
    await session.until(async () => (await cue.textContent())?.includes('저장된 어텐션'));
    assert.deepEqual(await call(videoPage, 'GET', `/videos/${saved.video_key}/subtitles`), subsBefore);
    assert.equal((await call(videoPage, 'GET', '/jobs', {page_url: saved.page_url})).body.cost_krw, 12.5);
    media.assertNoTranslation(); await session.observe('saved-reuse', assertions[0].expected, videoPage);
    await session.restart(); page = session.page;
    await library(); assert.deepEqual(await list(), organized);
    await session.observe('restart', assertions[1].expected);
    await click('영상 삭제', row('Saved attention'));
    const warning = await page.getByRole('alertdialog').textContent();
    assert.ok(warning.includes('Saved attention') && warning.includes('영구') && warning.includes('API 비용'));
    await click('취소'); assert.ok((await list()).videos.find(video => video.video_key === saved.video_key).saved);
    await session.observe('cancel-delete', assertions[2].expected);
    await click('영상 삭제', row('Saved attention')); await click('영구 삭제 확인');
    await session.until(async () => !(await list()).videos.some(video => video.video_key === saved.video_key));
    const after = await list(); assert.equal(after.playlists.length, 2);
    assert.ok(after.playlists.every(playlist => !playlist.entries.includes(saved.video_key)));
    assert.ok(after.videos.find(video => video.title === 'Unrelated systems').saved);
    await session.observe('delete', assertions[3].expected);
    await click('Systems topic (1)'); await click('영상 열기', row('Untranslated lecture'));
    const manualPage = session.platform === 'windows'
      ? await session.until(() => session.context.pages().find(p => p.url().includes('/untranslated.mp4'))) : page;
    await manualPage.locator('video').waitFor();
    await manualPage.getByRole('button', {name: '영상감지', exact: true}).waitFor();
    assert.equal((await call(manualPage, 'GET', '/jobs', {page_url: saved.page_url.replace('saved.mp4', 'untranslated.mp4')})).status, 404);
    media.assertNoTranslation(); await session.observe('manual-link', assertions[4].expected, manualPage);
  } finally {await media.close();}
}
