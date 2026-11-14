import assert from 'node:assert/strict';
import {spawn, execFileSync} from 'node:child_process';
import {mkdirSync, existsSync, writeFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {isDeepStrictEqual} from 'node:util';
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
const observe = (id, expected, facts) => observations.push({id, expected, actual: JSON.stringify(facts), pass: true});
const timeline = subtitles => subtitles.body.cues.map(cue => [cue.start_ms, cue.end_ms]);
const apiAssertions = [
  {id: 'reuse', expected: 'An upgraded saved translation keeps its cues and cost when reopened'},
  {id: 'persistence-delete', expected: 'Restart preserves playlists; confirmed deletion removes all memberships and preserves the other video'},
  {id: 'no-automatic-translation', expected: 'Adding and opening an untranslated entry creates no processing job'},
  {id: 'playlist-retention', expected: 'Removing a playlist preserves its saved translations and other playlists'},
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
    const jobBefore = await request('GET', '/jobs?page_url=' + encodeURIComponent(saved.page_url));
    await request('POST', '/browser/open', {url: saved.page_url});
    const reuse = await request('POST', '/jobs', {page_url: saved.page_url, title: 'Ignored', duration_ms: 5000});
    assert.equal(reuse.body.reused, true); assert.equal(reuse.status, 200);
    const reopenedCues = await request('GET', `/videos/${saved.video_key}/subtitles`);
    assert.deepEqual(reopenedCues, cues);
    const jobAfter = await request('GET', '/jobs?page_url=' + encodeURIComponent(saved.page_url));
    assert.equal(jobAfter.body.cost_krw, 12.5);
    observe(apiAssertions[0].id, apiAssertions[0].expected, {
      reused: reuse.body.reused, cost_before: jobBefore.body.cost_krw, cost_after: jobAfter.body.cost_krw,
      cue_times_before: timeline(cues), cue_times_after: timeline(reopenedCues),
      entire_subtitle_response_equal: isDeepStrictEqual(cues, reopenedCues)});
    for (const name of ['One', 'Two']) await change({action: 'create_playlist', name});
    const playlists = (await list()).playlists;
    for (const playlist of playlists) await change({action: 'add_entry', playlist_id: playlist.id, page_url: saved.page_url});
    const organized = await list(); await stopServer(); await startServer(data);
    const restarted = await list(); assert.deepEqual(restarted, organized);
    const cancellation = await request('POST', '/library/data', {action: 'delete_video', video_key: saved.video_key, title: saved.title, confirmed: false});
    const cancelled = await list();
    assert.equal(cancellation.status, 400); assert.ok(cancelled.videos.some(video => video.video_key === saved.video_key));
    await change({action: 'delete_video', video_key: saved.video_key, title: saved.title, confirmed: true});
    const after = await list(); assert.equal(after.playlists.length, 2); assert.ok(after.playlists.every(playlist => playlist.entries.length === 0));
    assert.equal(after.videos.length, 1); assert.equal(after.videos[0].title, 'Unrelated systems'); assert.ok(after.videos[0].saved);
    observe(apiAssertions[1].id, apiAssertions[1].expected, {
      before_restart: organized.playlists, after_restart: restarted.playlists,
      cancellation_status: cancellation.status, saved_video_present_after_cancel: cancelled.videos.some(video => video.video_key === saved.video_key),
      after_delete: {playlists: after.playlists, videos: after.videos}});
    const link = 'https://lectures.example.org/untranslated';
    await change({action: 'add_entry', playlist_id: playlists[0].id, page_url: link, title: 'Manual'});
    const openedLink = await request('POST', '/browser/open', {url: link});
    const linkJob = await request('GET', '/jobs?page_url=' + encodeURIComponent(link));
    assert.equal(openedLink.status, 200); assert.equal(linkJob.status, 404);
    observe(apiAssertions[2].id, apiAssertions[2].expected, {
      page_url: link, open_status: openedLink.status, job_lookup_status: linkJob.status, job_lookup_response: linkJob.body});
    const other = after.videos[0];
    const otherCues = await request('GET', `/videos/${other.video_key}/subtitles`);
    await change({action: 'add_entry', playlist_id: playlists[1].id, page_url: other.page_url});
    await change({action: 'delete_playlist', playlist_id: playlists[1].id});
    const retained = await list(); assert.equal(retained.playlists.length, 1);
    assert.ok(retained.videos.find(video => video.video_key === other.video_key).saved);
    const retainedCues = await request('GET', `/videos/${other.video_key}/subtitles`);
    assert.deepEqual(retainedCues, otherCues);
    observe(apiAssertions[3].id, apiAssertions[3].expected, {
      remaining_playlists: retained.playlists, remaining_videos: retained.videos,
      cue_times_before: timeline(otherCues), cue_times_after: timeline(retainedCues),
      entire_subtitle_response_equal: isDeepStrictEqual(otherCues, retainedCues)});
  }
  console.log('```local-evidence\n' + JSON.stringify({head, flow: `library-${kind}`,
    environment_id: process.env.WIKI_VERIFICATION_ENVIRONMENT, test_scope: process.env.WIKI_VERIFICATION_SCOPE,
    ...(kind === 'browser' ? {build_head: buildHead, browser_tool: 'playwright-chromium'} : {}), observations, requests, actions}) + '\n```');
} finally {if (browser) await browser.close(); await stopServer();}
