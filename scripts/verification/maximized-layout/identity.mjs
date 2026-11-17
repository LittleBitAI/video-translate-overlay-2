import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readFileSync, writeFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import os from 'node:os';

export const sha = bytes => createHash('sha256').update(bytes).digest('hex');
export const git = args => execFileSync('git', args, {encoding:'utf8'}).trim();
export const head = git(['rev-parse', 'HEAD']);
const scope = ['.', ':(exclude).wiki', ':(exclude)docs/evidence/maximized-layout'];
assert.equal(git(['diff', '--name-only', 'HEAD', '--', ...scope]), '', 'Tracked build inputs must be committed');
const require = createRequire(import.meta.url);
export const environment = {
  observedAt: new Date().toISOString(), platform:os.platform(), os:os.version(), release:os.release(), arch:os.arch(),
  node:process.version, playwright:require('playwright/package.json').version,
  rust:execFileSync('rustc', ['--version'], {encoding:'utf8'}).trim(),
};
export function inventory(out) {
  const tree = git(['ls-tree', '-r', 'HEAD', '--', ...scope]);
  const inputs = tree.split('\n').map(line => {
    const [metadata, path] = line.split('\t');
    return {path, blob:metadata.split(' ')[2]};
  });
  const result = {head, environment, treeSha256:sha(tree), inputs};
  writeFileSync(`${out}/build-inputs.json`, JSON.stringify(result, null, 2)+'\n');
  return result.treeSha256;
}
export function binary(path) { return {path, sha256:sha(readFileSync(path))}; }
export function inspect(pid, port, rootPid = pid) {
  return JSON.parse(execFileSync('pwsh', ['-NoProfile', '-File', 'scripts/verification/maximized-layout/inspect.ps1',
    '-AppPid', String(pid), '-Port', String(port), '-RootPid', String(rootPid)], {encoding:'utf8', windowsHide:true}));
}
