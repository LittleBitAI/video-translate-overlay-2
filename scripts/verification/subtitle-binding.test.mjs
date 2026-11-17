import assert from 'node:assert/strict';
import {test} from 'node:test';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {assertBound, sourceBinding, isInput} from './subtitle-binding.mjs';

const revision = name => execFileSync('git', ['rev-parse', name], {encoding:'utf8'}).trim();
const current = revision('HEAD');

test('a historical receipt without recorded source binding is refused', () => {
  const historical = JSON.parse(readFileSync('docs/evidence/fullscreen-subtitles/receipt.json'));
  assert.throws(() => assertBound(historical.sourceBinding, current), /Source binding is required/);
});

test('binding accepts identical inputs and refuses changed or incomplete inputs', () => {
  const valid = sourceBinding(current);
  const cases = [
    {name:'same revision', binding:valid, target:current, allowed:true},
    {name:'acceptance record only', binding:sourceBinding(revision('03b343d')), target:revision('f68351a'), allowed:true},
    {name:'before subtitle sizing changed', binding:sourceBinding(revision('a6e8e443256f')), target:current, allowed:false},
    {name:'missing binding', binding:undefined, target:current, allowed:false},
    {name:'different input digest', binding:{...valid, inputTreeSha256:'0'.repeat(64)}, target:current, allowed:false},
    {name:'different producer revision', binding:{...valid, head:revision('a6e8e443256f')}, target:current, allowed:false},
    {name:'missing exclusion policy', binding:{head:valid.head, inputTreeSha256:valid.inputTreeSha256}, target:current, allowed:false},
  ];
  for (const form of cases) {
    if (form.allowed) assert.doesNotThrow(() => assertBound(form.binding, form.target), form.name);
    else assert.throws(() => assertBound(form.binding, form.target), form.name);
  }
});

test('only the declared output packet, acceptance prose and wiki metadata are excluded', () => {
  for (const [path, included] of [
    ['src/overlay/geometry.ts',true], ['tests/ui/subtitles.test.svelte.ts',true],
    ['package-lock.json',true], ['src-tauri/build.rs',true],
    ['scripts/verification/subtitle-binding.mjs',true],
    ['docs/evidence/fullscreen-subtitles/scripts/check.mjs',true],
    ['docs/evidence/fullscreen-subtitles/scripts/launch.py',true],
    ['docs/evidence/fullscreen-subtitles/receipt.json',true],
    ['docs/evidence/fullscreen-subtitles/review-1/runtime.json',false],
    ['docs/evidence/fullscreen-subtitles/README.md',false], ['.wiki/corpus.json',false],
  ]) assert.equal(isInput(path), included, path);
});
