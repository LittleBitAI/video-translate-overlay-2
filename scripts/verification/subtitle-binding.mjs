import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';

export const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const git = args => execFileSync('git', args, {encoding:'utf8'}).trim();
export const exclusions = ['.wiki/', 'docs/evidence/fullscreen-subtitles/review-1/', 'docs/evidence/fullscreen-subtitles/README.md'];
export const isInput = path => !exclusions.some(excluded => excluded.endsWith('/') ? path.startsWith(excluded) : path === excluded);

export function sourceBinding(head = git(['rev-parse', 'HEAD'])) {
  assert.match(head, /^[a-f0-9]{40}$/);
  const entries = git(['ls-tree', '-r', head]).split('\n').filter(line => isInput(line.split('\t')[1]));
  return {head, inputTreeSha256:sha(entries.join('\n')), exclusions};
}

/** Evidence may cross revisions only when every retained source input is identical. */
export function assertBound(binding, target = git(['rev-parse', 'HEAD'])) {
  assert(binding, 'Source binding is required');
  assert.deepEqual(binding, sourceBinding(binding.head), 'Recorded producer inputs match Git');
  assert.equal(binding.inputTreeSha256, sourceBinding(target).inputTreeSha256, 'Evidence inputs match the reviewed HEAD');
}

export function assertClean() {
  const changed = git(['diff', '--name-only', 'HEAD']).split('\n').filter(path => path && isInput(path));
  const added = git(['ls-files', '--others', '--exclude-standard']).split('\n').filter(path => path && isInput(path));
  assert.deepEqual([...changed, ...added], [], 'Evidence producers and application inputs must be committed');
}
