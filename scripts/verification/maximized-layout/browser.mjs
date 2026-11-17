import assert from 'node:assert/strict';
import {mkdirSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {chromium} from 'playwright';
import {capture, measure} from './capture.mjs';
import {head, environment, inventory, binary, inspect, sha, binding} from './identity.mjs';

const base = process.env.LAYOUT_ORIGIN ?? 'http://127.0.0.1:8146';
const out = process.env.LAYOUT_OUTPUT ?? 'artifacts/maximized-layout/review-1/browser';
assert(process.env.APP_DATA_DIR, 'The owned disposable profile must be named');
mkdirSync(out, {recursive:true});
const treeSha256 = inventory(out);
const requests = [];
const assertions = [];
function check(name, expected, observed) {
  assertions.push({name, expected, observed});
  assert.deepEqual(observed, expected, name);
}
async function call(method, route, body) {
  const response = await fetch(base+route, {method, ...(body ? {headers:{'Content-Type':'application/json'}, body:JSON.stringify(body)} : {})});
  const result = {status:response.status, body:await response.json()};
  requests.push({method, route, ...(body ? {body} : {}), ...result});
  return result;
}
const build = await call('GET', '/build');
const buildBinding = binding(build.body.head);
check('unchanged application inputs at the core revision', [], buildBinding.changedBuildInputs);
const ownership = inspect(build.body.pid, new URL(base).port);
check('core executable', resolve('target/debug/vto-web.exe').toLowerCase(), ownership.image.toLowerCase());
const initial = await call('GET', '/library/data');
check('fresh isolated library', {videos:[], playlists:[]}, {videos:initial.body.videos, playlists:initial.body.playlists});
const created = await call('POST', '/library/data', {action:'create_playlist', name:'Layout fixtures'});
check('playlist creation status', 200, created.status);
const list = await call('GET', '/library/data');
check('playlist name', ['Layout fixtures'], list.body.playlists.map(p=>p.name));
const playlist = list.body.playlists[0].id;
for (let i=0; i<36; i++) {
  const result = await call('POST', '/library/data', {action:'add_entry', playlist_id:playlist,
    page_url:`${base}/lecture-${i}.mp4`, title:`Lecture ${i+1}: layout fixture`});
  check(`link ${i+1} accepted`, 200, result.status);
}
const library = await call('GET', '/library/data');
check('all links retained', 36, library.body.videos.length);
check('no translation created', true, library.body.videos.every(v=>!v.saved && !v.active));
const media = `${base}/layout.mp4`;
check('media opening status', 200, (await call('POST', '/browser/open', {url:media})).status);
for (const url of ['file:///outside/target.mp4', 'ftp://127.0.0.1/target.mp4', 'plain address']) {
  check('non-HTTP address denied', 400, (await call('POST', '/browser/open', {url})).status);
  const pointer = await call('GET', '/browser');
  check('rejected address preserves player and creates no job', {url:media, job:null}, {url:pointer.body.url, job:pointer.body.job});
}
check('settings readable', 200, (await call('GET', '/settings')).status);
const browser = await chromium.launch();
const sessions = [];
try {
  environment.chromium = browser.version();
  for (const [width,height] of [[1280,720],[1920,1080],[2560,1440],[384,854]]) {
    const page = await browser.newPage({viewport:{width,height}});
    await page.route('**/*', route => new URL(route.request().url()).origin === base ? route.continue() : route.abort());
    const network = [];
    const pending = [];
    page.on('response', response => {
      const url = new URL(response.url());
      if (url.origin === base && ['/browser','/browser/open','/library/data','/settings','/account'].includes(url.pathname)) {
        pending.push(response.json().then(body => network.push({method:response.request().method(), route:url.pathname, status:response.status(), body})));
      }
    });
    const errors = [];
    page.on('pageerror', error=>errors.push(error.message));
    await page.goto(base);
    const results = await capture(page, out, `${width}x${height}`, {coreHead:build.body.head, profile:sha(resolve(process.env.APP_DATA_DIR))});
    for (const r of results) {
      check('screen and core revisions match', build.body.head, r.screenHead);
      r.coreHead = build.body.head;
      r.profile = sha(resolve(process.env.APP_DATA_DIR));
    }
    await page.getByRole('link',{name:'영상',exact:true}).click();
    await page.locator('video').waitFor();
    const geometry = [];
    for (const notice of ['', 'Saved subtitles are ready. '.repeat(8)]) {
      if (notice) await page.evaluate(async ({notice,media}) => {
        const state = await (await fetch('/browser')).json();
        window.vtoReady({text:notice, url:media, account_generation:state.account_generation});
      }, {notice,media});
      await page.locator('#address').fill('file:///outside/target.mp4');
      await page.getByRole('button',{name:'열기',exact:true}).click();
      await page.locator('#address-error').waitFor();
      await page.waitForFunction(() => document.querySelector('video').getBoundingClientRect().bottom <= innerHeight);
      const state = await measure(page);
      const video = await page.locator('video').boundingBox();
      check('rejected address retains the media', media, await page.locator('video').getAttribute('src'));
      check('player bottom stays visible below messages', true, video.y+video.height<=height);
      geometry.push({notice:!!notice, ...state, video});
    }
    await page.screenshot({path:`${out}/${width}x${height}-player-messages.png`});
    await Promise.all(pending);
    check('no browser runtime errors', [], errors);
    sessions.push({viewport:[width,height], results, geometry, requests:network});
    await page.close();
  }
} finally { await browser.close(); }
writeFileSync(`${out}/receipt.json`, JSON.stringify({head, buildBinding, environment, treeSha256, origin:base,
  profile:sha(resolve(process.env.APP_DATA_DIR)), build:build.body, ownership, binary:binary('target/debug/vto-web.exe'),
  requests, assertions, sessions}, null, 2)+'\n');
console.log(`PASS: ${assertions.length} API/browser assertions, 4 viewports; core and screens ${build.body.head}`);
