import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {test} from 'node:test';
import {assertSameInputs,assertNoApplicationChanges,digest,productionInput,validateCatalog} from '../scripts/verification/sync-evidence.mjs';

test('evidence requires all four kinds, observations and matching receipt and source contents', () => {
  const files = [['src/component.rs','a'.repeat(40)]];
  const inputs = {files,sha256:digest(JSON.stringify(files))};
  const bytes = Buffer.from(JSON.stringify({observations:[{name:'observed result',pass:true,actual:{count:1}}]}));
  const entry = kind => ({kind,status:'executed',command:'bounded check',environment:{platform:'test'},
    source_revision:'b'.repeat(40),receipt:'docs/evidence/lan-sync/result.json',receipt_sha256:digest(bytes),inventory:'full'});
  const valid = {version:1,reviewed_revision:'c'.repeat(40),inventories:{full:inputs},executions:['api','browser','desktop','offline'].map(entry)};
  assert.deepEqual(validateCatalog(valid,()=>bytes,()=>inputs),['api','browser','desktop','offline']);
  const cases = [
    ['missing evidence kind',c=>c.executions.pop(),()=>bytes,()=>inputs],
    ['changed receipt',()=>{},()=>Buffer.from('{}'),()=>inputs],
    ['missing observations',c=>c.executions.forEach(e=>e.receipt_sha256=digest('{}')),()=>Buffer.from('{}'),()=>inputs],
    ['failed observation',c=>c.executions.forEach(e=>e.receipt_sha256=digest('{"observations":[{"name":"result","pass":false,"actual":0}]}')),()=>Buffer.from('{"observations":[{"name":"result","pass":false,"actual":0}]}'),()=>inputs],
    ['changed source',()=>{},()=>bytes,()=>({files:[['src/component.rs','d'.repeat(40)]],sha256:'e'.repeat(64)})],
    ['unexplained reuse',c=>c.executions[0].status='reused',()=>bytes,()=>inputs],
    ['missing source inventory',c=>delete c.inventories.full,()=>bytes,()=>inputs],
    ['misattributed source revision',()=>{},()=>bytes,revision=>revision==='HEAD'?inputs:({files:[],sha256:digest('[]')})],
  ];
  for (const [name,change,load,snapshot] of cases) {
    const catalog = structuredClone(valid);change(catalog);
    assert.throws(()=>validateCatalog(catalog,load,snapshot),name);
  }
});

test('source equivalence includes additions, removals and content changes across production input forms', () => {
  for (const name of ['src/player.ts','src-tauri/src/component.rs','src-tauri/gen/android/app/build.gradle.kts','Cargo.lock','index.html','.cargo/config.toml']) {
    assert(productionInput(name),name);
    const files = [[name,'a'.repeat(40)]], baseline = {files,sha256:digest(JSON.stringify(files))};
    for (const changed of [[],[[name,'b'.repeat(40)]],[...files,['src/new.ts','c'.repeat(40)]]]) {
      assert.throws(()=>assertSameInputs(baseline,{files:changed,sha256:digest(JSON.stringify(changed))}));
    }
  }
  for (const name of ['docs/design/backend.md','src-tauri/README.md','docs/evidence/lan-sync/catalog.json']) assert(!productionInput(name));
});

test('current-head evidence refuses dirty application inputs while permitting evidence and host metadata changes', () => {
  for (const name of ['src/player.ts','src-tauri/src/component.rs','Cargo.lock','src-tauri/gen/android/app/build.gradle.kts']) {
    assert.throws(()=>assertNoApplicationChanges([name]));
  }
  assert.doesNotThrow(()=>assertNoApplicationChanges(['docs/design/backend.md','docs/evidence/lan-sync/result.json','.wiki/graph.json','scripts/verification/sync-evidence.mjs']));
});

test('current lifecycle documentation describes integrated native ownership consistently', () => {
  const current = readFileSync('docs/design/local-network-sync.md','utf8').split('## Historical implementation evidence — 2026-11-15')[0];
  assert(current.includes('Native setup starts the controller'));
  assert(current.includes('ExitRequested'));
  const documents = [current,...['docs/design/backend.md','src-tauri/README.md','docs/design/google-auth-setup.md'].map(name=>readFileSync(name,'utf8'))];
  const stale = [/components are not yet started/i,/must still connect sign-in/i,/pending physical-device acceptance/i,/unfinished device acceptance/i];
  for (const document of documents) for (const claim of stale) assert(!claim.test(document),String(claim));
  const app = readFileSync('src-tauri/src/app.rs','utf8');
  assert(app.includes('Runtime::start(&core, cfg!(windows))'));
  assert(app.includes('runtime.stop()'));
});
