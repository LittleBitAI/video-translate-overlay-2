import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {test} from 'node:test';
import {assertSameInputs,assertNoApplicationChanges,digest,auditDigest,productionInput,validateCatalog,nativeAssertions} from '../scripts/verification/sync-evidence.mjs';

function fixture(){
  const files = [['src/component.rs','a'.repeat(40)]];
  const inputs = {files,sha256:digest(JSON.stringify(files))};
  const bytes = Buffer.from(JSON.stringify({observations:[{name:'observed result',pass:true,actual:{count:1}}]}));
  const source='scripts/verification/probe.mjs',output='docs/evidence/lan-sync/machine/result.json';
  const producer=Buffer.from('await device.open(); assert.equal(observed,7);');
  const machine=Buffer.from('{"value":7}');
  const audit=ids=>({sources:[{path:source,sha256:digest(producer)}],outputs:[{path:output,sha256:digest(machine)}],
    actions:[{id:'open-device',source,excerpt:'await device.open();'}],
    assertions:ids.map(id=>({id,source,excerpt:'assert.equal(observed,7);',output,field:['value'],operator:'equal',expected:7}))});
  const entry = (kind,native_flow) => ({kind,status:'executed',command:'bounded check',environment:{platform:'test'},
    ...(native_flow?{native_flow}:{}),audit:audit(native_flow?nativeAssertions[native_flow]:['measured-value']),
    source_revision:'b'.repeat(40),receipt:'docs/evidence/lan-sync/result.json',receipt_sha256:digest(bytes),inventory:'full'});
  const catalog={version:1,reviewed_revision:'c'.repeat(40),inventories:{full:inputs},executions:[...['api','browser','offline'].map(kind=>entry(kind)),...Object.keys(nativeAssertions).map(flow=>entry('desktop',flow))]};
  const filesByName=new Map([['docs/evidence/lan-sync/result.json',bytes],[source,producer],[output,machine]]);
  return {catalog,inputs,bytes,load:name=>{assert(filesByName.has(name),'Fixture file is unavailable');return filesByName.get(name);}};
}

test('evidence requires all kinds, native flows, observations and matching receipt and source contents', () => {
  const {catalog:valid,inputs,bytes,load}=fixture();
  assert.deepEqual(validateCatalog(valid,load,()=>inputs),['api','browser','desktop','offline']);
  const cases = [
    ['missing evidence kind',c=>c.executions=c.executions.filter(e=>e.kind!=='api'),load,()=>inputs],
    ['changed receipt',()=>{},()=>Buffer.from('{}'),()=>inputs],
    ['missing observations',c=>c.executions.forEach(e=>e.receipt_sha256=digest('{}')),()=>Buffer.from('{}'),()=>inputs],
    ['failed observation',c=>c.executions.forEach(e=>e.receipt_sha256=digest('{"observations":[{"name":"result","pass":false,"actual":0}]}')),()=>Buffer.from('{"observations":[{"name":"result","pass":false,"actual":0}]}'),()=>inputs],
    ['changed source',()=>{},load,()=>({files:[['src/component.rs','d'.repeat(40)]],sha256:'e'.repeat(64)})],
    ['unexplained reuse',c=>c.executions[0].status='reused',load,()=>inputs],
    ['missing source inventory',c=>delete c.inventories.full,load,()=>inputs],
    ['misattributed source revision',()=>{},load,revision=>revision==='HEAD'?inputs:({files:[],sha256:digest('[]')})],
    ['missing required native flow',c=>c.executions.pop(),load,()=>inputs],
  ];
  for (const [name,change,load,snapshot] of cases) {
    const catalog = structuredClone(valid);change(catalog);
    assert.throws(()=>validateCatalog(catalog,load,snapshot),name);
  }
});

test('pass-only summaries are refused across every evidence kind and required native flow',()=>{
  const {catalog,inputs,load}=fixture();
  for(const entry of catalog.executions){
    for(const actual of [true,'completed',{count:1},null]){
      const copy=structuredClone(catalog),summary=Buffer.from(JSON.stringify({observations:[{name:'result',pass:true,actual}]}));
      for(const item of copy.executions)item.receipt_sha256=digest(summary);
      delete copy.executions.find(e=>e.kind===entry.kind&&e.native_flow===entry.native_flow).audit;
      assert.throws(()=>validateCatalog(copy,name=>name==='docs/evidence/lan-sync/result.json'?summary:load(name),()=>inputs),/no inspectable execution audit/);
    }
  }
});

test('native audits require inspectable actions, executable assertions and matching observed machine output',()=>{
  const {catalog,inputs,load}=fixture();
  const cases=[
    ['missing producer',a=>a.sources=[]],['missing output',a=>a.outputs=[]],['missing action',a=>a.actions=[]],['missing assertion',a=>a.assertions=[]],
    ['changed producer',a=>a.sources[0].sha256='a'.repeat(64)],['changed output',a=>a.outputs[0].sha256='a'.repeat(64)],
    ['unavailable assertion',a=>a.assertions[0].excerpt='assert.equal(other,7);'],
    ['nonasserting excerpt',a=>a.assertions[0].excerpt='await device.open();'],
    ['wrong measurement',a=>a.assertions[0].expected=8],['missing measurement',a=>a.assertions[0].field=['unobserved']],
    ['summary flag',a=>a.assertions[0].field=['pass']],['unknown comparison',a=>a.assertions[0].operator='unknown'],
    ['outside source',a=>a.sources[0].path='outside/target.txt'],['outside output',a=>a.outputs[0].path='startup.rc'],
  ];
  for(const [name,mutate] of cases){const copy=structuredClone(catalog);mutate(copy.executions[3].audit);assert.throws(()=>validateCatalog(copy,load,()=>inputs),name);}
  for(const flow of Object.keys(nativeAssertions)){
    const copy=structuredClone(catalog);copy.executions.find(e=>e.native_flow===flow).audit.assertions.pop();
    assert.throws(()=>validateCatalog(copy,load,()=>inputs),/Missing native assertion/);
  }
});

test('bounded native predicates require their inspectable throwing failure path',()=>{
  const {catalog,inputs,load}=fixture();
  const entry=catalog.executions[3],source=entry.audit.sources[0].path;
  const text=Buffer.from("await poll(()=>device.position(),p=>p===7,'position'); throw new Error(label);");
  for(const item of catalog.executions){
    item.audit.sources[0].sha256=digest(text);
    item.audit.actions[0].excerpt="await poll(()=>device.position(),p=>p===7,'position');";
    for(const row of item.audit.assertions){row.excerpt=item.audit.actions[0].excerpt;row.failure_excerpt='throw new Error(label);';}
  }
  const read=name=>name===source?text:load(name);
  assert.doesNotThrow(()=>validateCatalog(catalog,read,()=>inputs));
  for(const missing of ['failure binding','failure implementation']){
    const copy=structuredClone(catalog);
    let sourceBytes=text;
    if(missing==='failure binding')delete copy.executions[3].audit.assertions[0].failure_excerpt;
    else{sourceBytes=Buffer.from(entry.audit.actions[0].excerpt);for(const item of copy.executions)item.audit.sources[0].sha256=digest(sourceBytes);}
    assert.throws(()=>validateCatalog(copy,name=>name===source?sourceBytes:load(name),()=>inputs),/executable diagnostic checks/);
  }
});

test('unavailable native attempts remain explicit failures and cannot supply passing evidence',()=>{
  const {catalog,inputs,load}=fixture(),file=catalog.executions[0].audit.outputs[0];
  catalog.unavailable_attempts=[{eligible_as_passing_evidence:false,output:file.path,sha256:file.sha256}];
  assert.doesNotThrow(()=>validateCatalog(catalog,load,()=>inputs));
  catalog.unavailable_attempts[0].eligible_as_passing_evidence=true;
  assert.throws(()=>validateCatalog(catalog,load,()=>inputs),/Unavailable attempts cannot supply passing evidence/);
});

test('audit text hashes survive Git line-ending normalization while still rejecting content changes',()=>{
  const {catalog,inputs,load}=fixture(),source=catalog.executions[0].audit.sources[0].path;
  const canonical=Buffer.from('await device.open();\nassert.equal(observed,7);\n');
  for(const entry of catalog.executions)entry.audit.sources[0].sha256=auditDigest(canonical);
  for(const bytes of [canonical,Buffer.from(canonical.toString().replaceAll('\n','\r\n'))]){
    assert.doesNotThrow(()=>validateCatalog(catalog,name=>name===source?bytes:load(name),()=>inputs));
  }
  const changed=Buffer.from(canonical.toString().replace('observed','other'));
  assert.throws(()=>validateCatalog(catalog,name=>name===source?changed:load(name),()=>inputs),/Diagnostic source changed/);
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
