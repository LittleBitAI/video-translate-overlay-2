// Read-only validation of retained subtitle evidence against the current checkout.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {assertBound, assertClean, sha} from './subtitle-binding.mjs';

const root = 'docs/evidence/fullscreen-subtitles/review-1/';
const read = name => JSON.parse(readFileSync(root + name, 'utf8'));
assertClean();
const runtime = read('runtime.json');
assertBound(runtime.sourceBinding);
for (const revision of [runtime.head, runtime.identity.head, runtime.identity.screen, runtime.browserCore.head, runtime.browserScreen]) {
  assert.equal(revision, runtime.sourceBinding.head, 'Every runtime producer reports the recorded revision');
}
assert.equal(runtime.native.length, 7);
assert.equal(runtime.browser.length, 3);
for (const observation of [...runtime.native, ...runtime.browser]) {
  const {image, subtitle, font, scroll} = observation;
  assert(subtitle.left >= image.left - 1 && subtitle.right <= image.left + image.width + 1);
  assert(subtitle.top >= image.top && subtitle.bottom <= image.top + image.height - Math.max(48, image.height * .1));
  assert(Math.abs(font - Math.max(14, Math.min(image.width * .03, image.height * .055))) < .1);
  assert(scroll[0] <= scroll[1] + 1);
  assert.equal(observation.overflow, false);
}
assert.equal(runtime.native.find(state => state.name === 'fullscreen-short').font, 57.6);
assert.deepEqual(runtime.browser.map(state => state.viewport), [[1280,720], [1920,1080], [384,854]]);
for (const owner of [runtime.ownership, runtime.browserOwnership]) {
  assert.equal(owner.ancestry.at(-1).pid, owner.rootPid);
  assert(owner.listeners.every(address => address === '127.0.0.1'));
}
for (const digest of [runtime.exe.sha256, runtime.browserExe.sha256, runtime.nativeCueDigest]) assert.match(digest, /^[a-f0-9]{64}$/);
const captures = read('captures.json');
assert.equal(Object.keys(captures).length, 10);
for (const [name, digest] of Object.entries(captures)) {
  assert.match(name, /^(native|browser)-[a-z0-9-]+\.png$/);
  assert.equal(sha(readFileSync(root + name)), digest, name);
}
const focused = read('focused.json');
assertBound(focused.sourceBinding);
assert.equal(focused.sourceBinding.head, runtime.sourceBinding.head);
assert.equal(focused.results.length, 3);
for (const result of focused.results) {
  assert.equal(result.exitCode, 0, result.command);
  assert.match(result.log, /^focused-[a-z]+\.log$/);
  assert.equal(sha(readFileSync(root + result.log)), result.sha256, result.command);
}
const head = execFileSync('git', ['rev-parse', 'HEAD'], {encoding:'utf8'}).trim();
console.log(`PASS: runtime, API, browser, desktop and offline evidence bound to HEAD ${head}; producer ${runtime.head}; 10 capture hashes verified`);
