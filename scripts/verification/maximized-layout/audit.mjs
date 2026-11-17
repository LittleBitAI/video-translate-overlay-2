// Read-only validation of this task's retained receipts against their Git revisions.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {binding,sha} from './identity.mjs';

const root='docs/evidence/maximized-layout/review-1/';
let total=0;
for (const area of ['browser','windows']) {
  const receipt=JSON.parse(readFileSync(root+area+'/receipt.json'));
  const inputs=JSON.parse(readFileSync(root+area+'/build-inputs.json'));
  const tree=execFileSync('git',['ls-tree','-r',receipt.head],{encoding:'utf8'}).trim().split('\n').filter(line=>{
    const path=line.split('\t')[1];
    return !path.startsWith('.wiki/') && !path.startsWith('docs/evidence/maximized-layout/');
  }).join('\n');
  assert.equal(sha(tree),receipt.treeSha256);
  assert.equal(inputs.treeSha256,receipt.treeSha256);
  assert.equal(inputs.head,receipt.head);
  assert.deepEqual(inputs.inputs,tree.split('\n').map(line=>{
    const [metadata,path]=line.split('\t');
    return {path,blob:metadata.split(' ')[2]};
  }));
  assert(receipt.requests.length>0 && receipt.assertions.length>0);
  for (const observation of receipt.assertions) {
    assert.deepEqual(observation.observed,observation.expected,observation.name);
    total++;
  }
  const core=area==='browser'?receipt.build.head:receipt.identity.core;
  assert.deepEqual(binding(core).changedBuildInputs,[]);
  if(area==='windows') {
    assert.equal(receipt.identity.screen,core);
    for(const source of receipt.launch.hostSources) assert.equal(sha(readFileSync(root+area+'/'+source.name)),source.sha256);
  } else for(const session of receipt.sessions) for(const view of session.results) {
    assert.equal(view.screenHead,core);
    assert.equal(view.coreHead,core);
    assert(view.profile);
  }
}
for(const revision of ['29f2e9e','f5277ed']) {
  const full=execFileSync('git',['rev-parse',revision],{encoding:'utf8'}).trim();
  assert.deepEqual(binding(full).changedBuildInputs,[]);
}
for(const revision of ['d9d1df6','355cbc7']) {
  const full=execFileSync('git',['rev-parse',revision],{encoding:'utf8'}).trim();
  assert.throws(()=>binding(full),/same application inputs/);
}
console.log(`PASS: ${total} retained assertions; both inventories and host sources match; build binding accepts 2 unchanged revisions and rejects 2 changed revisions.`);
