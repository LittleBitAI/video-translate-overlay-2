import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import path from 'node:path';
import {fixture} from './fixture.mjs';

export const assertions = [
  {id: 'saved-reuse', expected: 'Saved subtitles retain their times and cost without a translation request'},
  {id: 'restart', expected: 'Playlist names, order and memberships survive a process restart'},
  {id: 'cancel-delete', expected: 'Cancelling named permanent deletion preserves the saved translation'},
  {id: 'delete', expected: 'Confirmed deletion removes the video from every playlist and preserves unrelated data'},
  {id: 'playlist-retention', expected: 'Removing a playlist preserves its saved translations and other playlists'},
  {id: 'manual-link', expected: 'Opening an untranslated playlist entry leaves translation unstarted'},
];

export async function call(page, method, route, body = {}, accountGeneration) {
  return page.evaluate(async ({method, route, body, accountGeneration}) => {
    if (window.__TAURI_INTERNALS__) {
      const subs = /^\/videos\/([^/]+)\/subtitles$/.exec(route);
      const command = subs ? 'get_subtitles' : ({'GET /library/data': 'get_library', 'POST /library/data': 'library_action',
        'GET /jobs': 'get_job', 'POST /browser/open': 'browser_open'})[`${method} ${route}`];
      const args = subs ? {video_key: subs[1]} : method === 'POST' && route === '/library/data' ? {action: body} : {...body};
      if (accountGeneration === undefined) accountGeneration = (await window.__TAURI_INTERNALS__.invoke('get_google_account', {})).generation;
      args.account_generation = accountGeneration;
      try {return {status: 200, body: await window.__TAURI_INTERNALS__.invoke(command, args)};}
      catch (error) {return {status: error.status ?? 500, body: error};}
    }
    const suffix = method === 'GET' && Object.keys(body).length ? `?${new URLSearchParams(body)}` : '';
    const response = await fetch(route + suffix, {method,
      ...(method !== 'GET' ? {headers: {'Content-Type': 'application/json'}, body: JSON.stringify(body)} : {})});
    return {status: response.status, body: await response.json()};
  }, {method, route, body, accountGeneration});
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
    // A card by its title; a playlist row (rail item from 768 px, accordion row below) by its name.
    const row = title => page.locator('li[data-video-key]').filter({hasText: title});
    const playlistRow = name => page.locator('ul[aria-label="재생목록"] li > div > button')
      .filter({hasText: new RegExp(`^\\s*${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')} 영상 \\d+개\\s*$`)});
    /** Shows a playlist's videos; tapping an open accordion row again would close it. */
    const select = async name => {
      const button = playlistRow(name);
      if (await button.getAttribute('aria-expanded') === 'true' || await button.getAttribute('aria-current') === 'true') return;
      await button.click();
      await page.locator('ul[aria-label="재생목록"] li > div > button[aria-expanded="true"], ul[aria-label="재생목록"] li > div > button[aria-current="true"]')
        .filter({hasText: name}).waitFor();
    };
    /** Turns edit mode on or off with the 편집 / 완료 toggle. */
    const editing = async on => {
      const done = page.getByRole('button', {name: '완료', exact: true});
      if (await done.count() !== (on ? 1 : 0)) await click(on ? '편집' : '완료');
      await page.getByRole('button', {name: on ? '완료' : '편집', exact: true}).waitFor();
    };
    const addPanel = async () => {
      const panel = page.getByRole('region', {name: '추가', exact: true});
      if (!await panel.count()) await click('추가');
      await panel.waitFor();
      return panel;
    };
    await library();
    const before = await list(); assert.equal(before.videos.length, 2); assert.ok(before.videos.every(video => video.saved));
    const saved = before.videos.find(video => video.title === 'Saved attention');
    const subsBefore = await call(page, 'GET', `/videos/${saved.video_key}/subtitles`);
    const costBefore = await call(page, 'GET', '/jobs', {page_url: saved.page_url});
    assert.equal(costBefore.body.cost_krw, 12.5);
    session.record('saved-before.json', {library: before, subtitles: subsBefore, job: costBefore});
    for (const name of ['Deep learning', 'Systems']) {
      // + 새 재생목록 selects the playlist it creates, and the panel stays open on it.
      let panel = await addPanel();
      await panel.getByLabel('새 재생목록 이름').fill(name); await click('+ 새 재생목록', panel);
      await page.locator('ul[aria-label="재생목록"] button[aria-expanded="true"], ul[aria-label="재생목록"] button[aria-current="true"]').filter({hasText: name}).waitFor();
      panel = await addPanel();
      await click('Saved attention 추가', panel);
      await session.until(async () => (await list()).playlists.find(p => p.name === name)?.entries.length === 1);
    }
    let panel = await addPanel();
    await click('Unrelated systems 추가', panel);
    await session.until(async () => (await list()).playlists[1].entries.length === 2);
    await panel.getByLabel('영상 링크', {exact: true}).fill(saved.page_url.replace('saved.mp4', 'untranslated.mp4'));
    await panel.getByLabel('영상 이름 (선택)').fill('Untranslated lecture'); await click('링크 추가', panel);
    await session.until(async () => (await list()).playlists[1].entries.length === 3);
    await click('닫기', panel);
    await editing(true);
    await select('Systems');
    await page.getByLabel('재생목록 이름', {exact: true}).fill('Systems topic'); await click('이름 바꾸기');
    await playlistRow('Systems topic').waitFor();
    await click('Systems topic 재생목록 위로');
    await session.until(async () => (await list()).playlists[0].name === 'Systems topic');
    // One drag over two places is one multi-place move request.
    await row('Untranslated lecture').getByTitle('끌어서 옮기기').dragTo(row('Saved attention'));
    await session.until(async () => (await list()).playlists[0].entries[0] === (await list()).videos.find(v => v.title === 'Untranslated lecture')?.video_key);
    await click('목록에서 빼기', row('Unrelated systems'));
    await session.until(async () => (await list()).playlists[0].entries.length === 2);
    await editing(false);
    const organized = await list();
    const untranslated = organized.videos.find(video => video.title === 'Untranslated lecture');
    assert.ok(organized.playlists[0].entries.indexOf(untranslated.video_key) < organized.playlists[0].entries.indexOf(saved.video_key));
    session.record('organized.json', organized);
    await click('Saved attention 영상 열기', row('Saved attention'));
    const videoPage = session.platform === 'browser' ? page : await session.pageFor(saved.page_url);
    await videoPage.locator('video').waitFor();
    await videoPage.evaluate(() => {const video = document.querySelector('video'); video.currentTime = 1; video.dispatchEvent(new Event('timeupdate'));});
    const cue = videoPage.locator(session.platform === 'browser' ? '[data-feature="watch-subtitles"]' : '[data-vto="subtitle"]');
    await session.until(async () => (await cue.textContent())?.includes('저장된 어텐션'));
    assert.deepEqual(await call(videoPage, 'GET', `/videos/${saved.video_key}/subtitles`, {}, before.account_generation), subsBefore);
    assert.equal((await call(videoPage, 'GET', '/jobs', {page_url: saved.page_url}, before.account_generation)).body.cost_krw, 12.5);
    media.assertNoTranslation(); await session.observe('saved-reuse', assertions[0].expected, videoPage);
    await session.restart(); page = session.page;
    await library(); assert.deepEqual(await list(), organized);
    await session.observe('restart', assertions[1].expected);
    await select('전체 영상'); await editing(true);
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
    session.record('deleted.json', after);
    await session.observe('delete', assertions[3].expected);
    await select('Deep learning');
    panel = await addPanel();
    await click('Unrelated systems 추가', panel);
    await session.until(async () => (await list()).playlists.find(playlist => playlist.name === 'Deep learning')?.entries.length === 1);
    await click('닫기', panel); await editing(true);
    const other = (await list()).videos.find(video => video.title === 'Unrelated systems');
    const otherCues = await call(page, 'GET', `/videos/${other.video_key}/subtitles`);
    await click('재생목록 삭제');
    assert.ok((await page.getByRole('alertdialog').textContent()).includes('Deep learning'));
    await click('영구 삭제 확인');
    await session.until(async () => (await list()).playlists.length === 1);
    const retained = await list(); assert.equal(retained.playlists[0].name, 'Systems topic');
    assert.ok(retained.videos.find(video => video.video_key === other.video_key).saved);
    assert.deepEqual(await call(page, 'GET', `/videos/${other.video_key}/subtitles`), otherCues);
    session.record('playlist-removed.json', retained);
    await session.observe('playlist-retention', assertions[4].expected);
    await editing(false); await select('Systems topic');
    await click('Untranslated lecture 영상 열기', row('Untranslated lecture'));
    const manualPage = session.platform === 'browser' ? page : await session.pageFor(saved.page_url.replace('saved.mp4', 'untranslated.mp4'));
    await manualPage.locator('video').waitFor();
    await manualPage.getByRole('button', {name: '영상감지', exact: true}).waitFor();
    assert.equal((await call(manualPage, 'GET', '/jobs', {page_url: saved.page_url.replace('saved.mp4', 'untranslated.mp4')}, after.account_generation)).status, 404);
    media.assertNoTranslation(); await session.observe('manual-link', assertions[5].expected, manualPage);
  } finally {await media.close();}
}
