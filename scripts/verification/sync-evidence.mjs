import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';

export const digest = bytes => createHash('sha256').update(bytes).digest('hex');
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

export function validateCatalog(catalog, load, snapshot, candidate = 'HEAD') {
  assert.equal(catalog.version,1);
  assert.match(catalog.reviewed_revision,/^[0-9a-f]{40}$/);
  const kinds = new Set();
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
    const inputs = catalog.inventories[entry.inventory];
    assert(inputs,'Missing source inventory');
    assertSameInputs(inputs,snapshot(entry.source_revision,entry.paths ?? null));
    assertSameInputs(inputs,snapshot(candidate,entry.paths ?? null));
    assert(entry.status !== 'reused' || entry.reuse_reason,'Reused evidence needs an impact explanation');
    kinds.add(entry.kind);
  }
  for (const kind of ['api','browser','desktop','offline']) assert(kinds.has(kind),`Missing ${kind} evidence`);
  return [...kinds].sort();
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  assertCleanApplication();
  const revision = process.argv[2] ?? 'HEAD';
  const catalog = JSON.parse(readFileSync('docs/evidence/lan-sync/catalog.json','utf8'));
  const kinds = validateCatalog(catalog, name => readFileSync(name), sourceSnapshot, revision);
  console.log(`Sync evidence passed for ${git(['rev-parse',revision])}: ${kinds.join(', ')}; ${catalog.executions.length} inspectable executions`);
}
