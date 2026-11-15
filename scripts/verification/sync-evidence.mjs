import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';

export const digest = bytes => createHash('sha256').update(bytes).digest('hex');
// Git stores these public text artifacts with LF regardless of the local checkout.
export const auditDigest = bytes => digest(bytes.toString('utf8').replaceAll('\r\n','\n'));
const git = args => execFileSync('git', args, {encoding:'utf8', windowsHide:true}).trim();
export const productionInput = name => !name.endsWith('.md') && (
  /^(?:src\/|src-tauri\/|\.cargo\/|\.config\/)/.test(name)
  || /^(?:Cargo\.(?:toml|lock)|package(?:-lock)?\.json|index\.html|vite(?:\.overlay)?\.config\.ts|\.gitattributes)$/.test(name));

export function assertNoApplicationChanges(names) {
  assert(!names.some(productionInput),'Uncommitted application inputs cannot supply current-head evidence');
}
export function assertCleanApplication() {
  const names = [...git(['diff','--name-only','-z','HEAD']).split('\0'),...git(['ls-files','--others','--exclude-standard','-z']).split('\0')].filter(Boolean);
  assertNoApplicationChanges(names);
}

// Git object identities cover file contents and the inventory, including added inputs.
export function sourceSnapshot(revision, paths = null) {
  const files = git(['ls-tree','-r',revision]).split('\n').filter(Boolean).map(line => {
    const [metadata,name] = line.split('\t'); return [name,metadata.split(' ')[2]];
  }).filter(([name]) => paths ? paths.includes(name) : productionInput(name));
  if (paths) assert.equal(files.length,paths.length,'A declared source input is missing');
  assert(files.length > 0,'Empty source inventory');
  return {files,sha256:digest(JSON.stringify(files))};
}

export function assertSameInputs(recorded,current) {
  assert.deepEqual(current.files,recorded.files,'Evidence inputs differ from the candidate');
  assert.equal(recorded.sha256,digest(JSON.stringify(recorded.files)),'Invalid source inventory digest');
  assert.equal(current.sha256,recorded.sha256,'Evidence source digest differs');
}

export const nativeAssertions = {
  playback:['pc-position','phone-resume','controlled-player','pc-reopen'],
  deletion_restart:['cancel','memberships','stale-request','windows-restart','android-restart','unrelated-cues'],
  screen_off:['display-off','noninteractive','foreground-service','delivery','network'],
  account_isolation:['different-accounts','windows-isolation','android-isolation','old-queue','matching-account','separate-stores','restore-original'],
};

function auditPath(name,source) {
  assert(typeof name==='string'&&!name.split('/').includes('..'),'Invalid audit path');
  assert(source
    ? name==='scripts/live.mjs'||/^(?:scripts\/verification\/|tests\/|src-tauri\/src\/|docs\/evidence\/lan-sync\/diagnostics\/)[A-Za-z0-9_./-]+$/.test(name)
    : /^docs\/evidence\/lan-sync\/machine\/[a-z0-9-]+\.(?:json|log)$/.test(name),'Audit file is outside its repository scope');
}

// A success flag is an index entry, not evidence: retain the producer and its output.
export function validateAudit(entry,load) {
  const audit=entry.audit;assert(audit,'Receipt has no inspectable execution audit');
  assert(audit.sources?.length&&audit.outputs?.length&&audit.actions?.length&&audit.assertions?.length,'Incomplete execution audit');
  const sources=new Map(),outputs=new Map();
  for(const file of audit.sources){
    auditPath(file.path,true);assert.match(file.sha256,/^[0-9a-f]{64}$/);
    const bytes=load(file.path);assert.equal(auditDigest(bytes),file.sha256,'Diagnostic source changed');sources.set(file.path,bytes.toString('utf8').replaceAll('\r\n','\n'));
  }
  for(const file of audit.outputs){
    auditPath(file.path,false);assert.match(file.sha256,/^[0-9a-f]{64}$/);
    const bytes=load(file.path);assert.equal(auditDigest(bytes),file.sha256,'Machine output changed');
    outputs.set(file.path,file.path.endsWith('.json')?JSON.parse(bytes):bytes.toString('utf8'));
  }
  const excerpt=(row,assertion)=>{
    assert(row.id&&typeof row.excerpt==='string'&&row.excerpt.length>8,'Missing action or assertion');
    const source=sources.get(row.source);assert(source&&source.includes(row.excerpt),'Referenced diagnostic operation is unavailable');
    if(assertion){
      const direct=/\bassert(?:_eq|_ne)?(?:!|[.(\s])/.test(row.excerpt);
      const bounded=/\bawait poll\(/.test(row.excerpt)&&row.excerpt.includes('=>')
        &&row.failure_excerpt==='throw new Error(label);'&&source.includes(row.failure_excerpt);
      assert(direct||bounded,'Assertion must reference executable diagnostic checks');
    }
  };
  for(const action of audit.actions)excerpt(action,false);
  const ids=new Set();
  for(const row of audit.assertions){
    excerpt(row,true);assert(!ids.has(row.id),'Duplicate audit assertion');ids.add(row.id);
    assert(outputs.has(row.output),'Assertion has no machine output');
    assert(Array.isArray(row.field),'Missing output field');let actual=outputs.get(row.output);
    for(const field of row.field){
      assert((typeof field==='string'||Number.isInteger(field))&& !['__proto__','prototype','constructor','pass'].includes(field),'Invalid output field');
      assert(actual!==null&&typeof actual==='object'&&Object.hasOwn(actual,field),'Observed output field is absent');actual=actual[field];
    }
    assert(Object.hasOwn(row,'expected'),'Missing expected output');
    if(row.operator==='equal')assert.deepEqual(actual,row.expected,'Observed diagnostic output differs');
    else if(row.operator==='contains')assert(typeof actual==='string'&&typeof row.expected==='string'&&actual.includes(row.expected),'Expected machine output is absent');
    else assert.fail('Unsupported audit assertion');
  }
  if(entry.native_flow){
    assert(entry.kind==='desktop'&&Object.hasOwn(nativeAssertions,entry.native_flow),'Unknown required native flow');
    for(const id of nativeAssertions[entry.native_flow])assert(ids.has(id),`Missing native assertion ${id}`);
  }
}

export function validateCatalog(catalog, load, snapshot, candidate = 'HEAD') {
  assert.equal(catalog.version,1);
  assert.match(catalog.reviewed_revision,/^[0-9a-f]{40}$/);
  const kinds = new Set(),flows = new Set();
  for (const entry of catalog.executions) {
    assert(['api','browser','desktop','offline'].includes(entry.kind));
    assert(['executed','reused'].includes(entry.status));
    assert(entry.command && entry.environment && Object.keys(entry.environment).length);
    assert.match(entry.source_revision,/^[0-9a-f]{40}$/);
    assert.match(entry.receipt,/^docs\/evidence\/lan-sync\/[a-z0-9-]+\.json$/);
    const bytes = load(entry.receipt);
    assert.equal(digest(bytes),entry.receipt_sha256,'Receipt contents changed');
    const receipt = JSON.parse(bytes);
    assert(receipt.observations?.length > 0,'Evidence has no observations');
    assert(receipt.observations.every(row => row.name && row.pass === true && row.actual !== undefined),'Missing or failed observation');
    validateAudit(entry,load);
    if(entry.native_flow)flows.add(entry.native_flow);
    const inputs = catalog.inventories[entry.inventory];
    assert(inputs,'Missing source inventory');
    assertSameInputs(inputs,snapshot(entry.source_revision,entry.paths ?? null));
    assertSameInputs(inputs,snapshot(candidate,entry.paths ?? null));
    assert(entry.status !== 'reused' || entry.reuse_reason,'Reused evidence needs an impact explanation');
    kinds.add(entry.kind);
  }
  for (const kind of ['api','browser','desktop','offline']) assert(kinds.has(kind),`Missing ${kind} evidence`);
  for(const flow of Object.keys(nativeAssertions))assert(flows.has(flow),`Missing required native flow ${flow}`);
  for(const attempt of catalog.unavailable_attempts??[]){
    assert(attempt.eligible_as_passing_evidence===false,'Unavailable attempts cannot supply passing evidence');
    auditPath(attempt.output,false);
    assert.equal(auditDigest(load(attempt.output)),attempt.sha256,'Unavailable-attempt output changed');
  }
  return [...kinds].sort();
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  assertCleanApplication();
  const revision = process.argv[2] ?? 'HEAD';
  const catalog = JSON.parse(readFileSync('docs/evidence/lan-sync/catalog.json','utf8'));
  const kinds = validateCatalog(catalog, name => readFileSync(name), sourceSnapshot, revision);
  console.log(`Sync evidence passed for ${git(['rev-parse',revision])}: ${kinds.join(', ')}; ${catalog.executions.length} inspectable executions; ${Object.keys(nativeAssertions).length} required native flows have source/output assertion bindings`);
}
