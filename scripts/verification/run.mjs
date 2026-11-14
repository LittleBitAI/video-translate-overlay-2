import assert from 'node:assert/strict';
import {spawn, execFileSync} from 'node:child_process';
import {mkdirSync, existsSync, writeFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright';
import scenario, {assertions} from './library-scenario.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const kind = process.argv[2]; assert.ok(['api', 'browser'].includes(kind));
const head = process.env.WIKI_VERIFICATION_HEAD;
assert.match(head ?? '', /^[a-f0-9]{40}$/);
const artifact = process.env.WIKI_VERIFICATION_ARTIFACTS;
assert.ok(artifact); mkdirSync(artifact, {recursive: true});
const base = 'http://127.0.0.1:8136';
const binary = path.join(root, 'target/debug/vto-web' + (process.platform === 'win32' ? '.exe' : ''));
assert.ok(existsSync(binary), 'Build the verification binaries before configuring local verification');
let server, browser, page, directory, buildHead;
const requests = [], actions = [], observations = [];
const until = async fn => {
  const end = Date.now() + 30000; let last;
  do {try {const value = await fn(); if (value) return value;} catch (error) {last = error;}
    await new Promise(resolve => setTimeout(resolve, 100));
  } while (Date.now() < end);
  throw new Error(`Readiness timed out: ${last?.message ?? 'no result'}`);
};
async function stopServer() {
  if (!server || server.exitCode !== null) return;
  const stopped = new Promise(resolve => server.once('exit', resolve)); server.kill();
  await Promise.race([stopped, new Promise((_, reject) => setTimeout(() => reject(new Error('Server cleanup timed out')), 5000))]);
}
async function startServer(data) {
  directory = data;
  server = spawn(binary, ['--stand-ins'], {cwd: root, stdio: 'ignore', windowsHide: true,
    env: {...process.env, PORT: '8136', APP_DATA_DIR: data, VTO_DIST: path.join(root, 'dist')}});
  let spawnError; server.on('error', error => {spawnError = error;});
  await until(async () => {
    if (spawnError) throw spawnError;
    assert.equal(server.exitCode, null, 'Owned server exited before readiness');
    const response = await fetch(`${base}/build`);
    if (!response.ok) return false;
    const identity = await response.json();
    return identity.pid === server.pid && identity;
  });
  const identity = await (await fetch(`${base}/build`)).json();
  assert.equal(identity.pid, server.pid, 'API belongs to another process');
  assert.equal(identity.head, head, 'Running API is from another commit');
}
async function request(method, route, body) {
  const response = await fetch(base + route, {method,
    ...(body ? {headers: {'Content-Type': 'application/json'}, body: JSON.stringify(body)} : {})});
  requests.push({method, url: base + route, status: response.status});
  return {status: response.status, body: await response.json()};
}
const change = async body => {const result = await request('POST', '/library/data', body); assert.equal(result.status, 200); return result.body;};
const list = async () => (await request('GET', '/library/data')).body;
const observe = (id, expected) => observations.push({id, expected, actual: expected, pass: true});
const apiAssertions = [
  {id: 'reuse', expected: 'An upgraded saved translation keeps its cues and cost when reopened'},
  {id: 'persistence-delete', expected: 'Restart preserves playlists; confirmed deletion removes all memberships and preserves the other video'},
  {id: 'no-automatic-translation', expected: 'Adding and opening an untranslated entry creates no processing job'},
];
try {
  if (kind === 'browser') {
    const session = {root, artifact, platform: 'browser', until,
      get page() {return page;}, get context() {return browser.contexts()[0];},
      async start(data) {await startServer(data); browser = await chromium.launch({headless: true}); await this.open();},
      async open() {
        page = await browser.newPage({viewport: {width: 1280, height: 800}});
        page.on('response', response => {
          if (response.url().startsWith(base + '/')) requests.push({method: response.request().method(), url: response.url(), status: response.status()});
        });
        await page.goto(base);
        buildHead = await page.locator('meta[name="wiki-build-head"]').getAttribute('content');
        assert.equal(buildHead, head, 'Running screen is from another commit');
      },
      async restart() {await browser.close(); await stopServer(); await startServer(directory); browser = await chromium.launch({headless: true}); await this.open();},
      async observe(id, actual, current = page) {
        const expected = assertions.find(assertion => assertion.id === id)?.expected; assert.equal(actual, expected);
        await current.screenshot({path: path.join(artifact, `${id}.png`)});
        observations.push({id, expected, actual, pass: true});
        actions.push({action: id, expected, actual});
      },
      record(name, data) {assert.match(name, /^[a-z0-9-]+\.json$/); writeFileSync(path.join(artifact, name), JSON.stringify(data));}};
    await scenario(session);
  } else {
    const data = path.join(artifact, 'api-data');
    execFileSync(process.env.VTO_VERIFICATION_PYTHON ?? 'python', [path.join(root, 'scripts/verification/seed.py'), data, '127.0.0.1', '8137'], {windowsHide: true});
    await startServer(data);
    const before = await list(); assert.equal(before.videos.length, 2);
    const saved = before.videos.find(video => video.title === 'Saved attention');
    const cues = await request('GET', `/videos/${saved.video_key}/subtitles`);
    await request('POST', '/browser/open', {url: saved.page_url});
    const reuse = await request('POST', '/jobs', {page_url: saved.page_url, title: 'Ignored', duration_ms: 5000});
    assert.equal(reuse.body.reused, true); assert.equal(reuse.status, 200);
    assert.deepEqual(await request('GET', `/videos/${saved.video_key}/subtitles`), cues);
    assert.equal((await request('GET', '/jobs?page_url=' + encodeURIComponent(saved.page_url))).body.cost_krw, 12.5);
    observe(apiAssertions[0].id, apiAssertions[0].expected);
    for (const name of ['One', 'Two']) await change({action: 'create_playlist', name});
    const playlists = (await list()).playlists;
    for (const playlist of playlists) await change({action: 'add_entry', playlist_id: playlist.id, page_url: saved.page_url});
    const organized = await list(); await stopServer(); await startServer(data); assert.deepEqual(await list(), organized);
    const cancellation = await request('POST', '/library/data', {action: 'delete_video', video_key: saved.video_key, title: saved.title, confirmed: false});
    assert.equal(cancellation.status, 400); assert.ok((await list()).videos.some(video => video.video_key === saved.video_key));
    await change({action: 'delete_video', video_key: saved.video_key, title: saved.title, confirmed: true});
    const after = await list(); assert.equal(after.playlists.length, 2); assert.ok(after.playlists.every(playlist => playlist.entries.length === 0));
    assert.equal(after.videos.length, 1); assert.equal(after.videos[0].title, 'Unrelated systems'); assert.ok(after.videos[0].saved);
    observe(apiAssertions[1].id, apiAssertions[1].expected);
    const link = 'https://lectures.example.org/untranslated';
    await change({action: 'add_entry', playlist_id: playlists[0].id, page_url: link, title: 'Manual'});
    assert.equal((await request('POST', '/browser/open', {url: link})).status, 200);
    assert.equal((await request('GET', '/jobs?page_url=' + encodeURIComponent(link))).status, 404);
    observe(apiAssertions[2].id, apiAssertions[2].expected);
  }
  console.log('```local-evidence\n' + JSON.stringify({head, flow: `library-${kind}`,
    environment_id: process.env.WIKI_VERIFICATION_ENVIRONMENT, test_scope: process.env.WIKI_VERIFICATION_SCOPE,
    ...(kind === 'browser' ? {build_head: buildHead, browser_tool: 'playwright-chromium'} : {}), observations, requests, actions}) + '\n```');
} finally {if (browser) await browser.close(); await stopServer();}
