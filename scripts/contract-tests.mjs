// Runs every contract test and writes one JUnit report: `node scripts/contract-tests.mjs <report.xml>`.
//
// 1. Sets up the tools: cargo-nextest (an installed `cargo nextest`, else the pinned release
//    from GitHub in target/tools/nextest; failing both, the test binaries run directly and
//    the report keeps nextest's shape) and Playwright's Chromium.
// 2. Builds the app's `vto-contract-host` binary; the backend tests reach the app through it.
// 3. Runs the backend tests with cargo-nextest and the overlay tests with Vitest. A failing
//    tool does not stop the other one.
// 4. Merges both reports, giving each Rust testcase the `file` it lives in.
// Exits non-zero when anything failed.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const report = process.argv[2];
if (!report) {
  console.error('usage: node scripts/contract-tests.mjs <report.xml>');
  process.exit(2);
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const run = (cmd, args, env = {}) =>
  spawnSync(cmd, args, { cwd: root, stdio: 'inherit', env: { ...process.env, ...env } }).status ?? 1;

const win = process.platform === 'win32';
// Windows' own curl and tar (bsdtar reads zip); a POSIX tar earlier on PATH would not.
const sys = (tool) => (win ? join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', `${tool}.exe`) : tool);

const NEXTEST = '0.9.151';
/** `cargo`, a downloaded cargo-nextest, or null when neither can be had. */
function nextest() {
  if (spawnSync('cargo', ['nextest', '--version'], { cwd: root, stdio: 'ignore' }).status === 0) return 'cargo';
  const dir = join(root, 'target', 'tools', 'nextest');
  const exe = join(dir, win ? 'cargo-nextest.exe' : 'cargo-nextest');
  if (!existsSync(exe)) {
    const asset = { win32: 'x86_64-pc-windows-msvc.zip', linux: 'x86_64-unknown-linux-gnu.tar.gz', darwin: 'universal-apple-darwin.tar.gz' }[process.platform];
    const archive = join(dir, `cargo-nextest-${NEXTEST}.${asset.endsWith('.zip') ? 'zip' : 'tar.gz'}`);
    const url = `https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-${NEXTEST}/cargo-nextest-${NEXTEST}-${asset}`;
    mkdirSync(dir, { recursive: true });
    // Resumes a partial download; gives up on a link slower than 1 KiB/s for a minute.
    const got = run(sys('curl'), ['-fsSL', '-C', '-', '--retry', '2', '--speed-time', '60', '--speed-limit', '1024', '-o', archive, url]);
    if (got !== 0 || run(sys('tar'), ['-xf', archive, '-C', dir]) !== 0) return null;
  }
  return existsSync(exe) ? exe : null;
}

const xml = (s) => String(s).replace(/[<>&"]/g, (c) => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;' })[c]);

/**
 * Without nextest: runs the same test binaries and writes nextest's JUnit shape, so a
 * testcase keeps its suite, classname (`<package>::<binary>`) and name either way.
 */
function runWithoutNextest(env, out) {
  console.error('cargo-nextest is unavailable: running the test binaries directly.');
  const opts = { cwd: root, env: { ...process.env, ...env }, encoding: 'utf8', maxBuffer: 1 << 28 };
  const build = spawnSync('cargo', ['test', '--workspace', '--no-run', '--message-format=json'], { ...opts, stdio: ['ignore', 'pipe', 'inherit'] });
  let failed = build.status !== 0;
  const suites = [];
  for (const line of (build.stdout ?? '').split('\n')) {
    let m;
    try { m = JSON.parse(line); } catch { continue; }
    if (m.reason !== 'compiler-artifact' || !m.executable || !m.profile?.test) continue;
    const pkg = readFileSync(m.manifest_path, 'utf8').match(/^\s*name\s*=\s*"([^"]+)"/m)?.[1] ?? m.target.name;
    const kind = m.target.kind[0];
    const id = kind === 'test' ? `${pkg}::${m.target.name}` : ['lib', 'proc-macro'].includes(kind) ? pkg : `${pkg}::${kind}/${m.target.name}`;
    const r = spawnSync(m.executable, [], { ...opts, cwd: dirname(m.manifest_path), stdio: ['ignore', 'pipe', 'pipe'] });
    const text = `${r.stdout ?? ''}`.replace(/\r\n/g, '\n');
    process.stdout.write(text);
    process.stderr.write(r.stderr ?? '');
    const details = {};
    for (const d of text.matchAll(/^---- (\S+) stdout ----\n([\s\S]*?)(?=^---- \S+ stdout ----$|^failures:$)/gm)) details[d[1]] = d[2].trim();
    const cases = [...text.matchAll(/^test (\S+) \.\.\. (ok|FAILED|ignored)/gm)];
    if (r.status !== 0 && !cases.some((c) => c[2] === 'FAILED')) failed = true;
    const body = cases.map(([, name, result]) => {
      if (result === 'FAILED') failed = true;
      const inner = result === 'FAILED'
        ? `<failure type="test failure" message="${xml((details[name] ?? 'failed').split('\n')[0])}">${xml(details[name] ?? '')}</failure>`
        : result === 'ignored' ? '<skipped/>' : '';
      return `<testcase name="${xml(name)}" classname="${xml(id)}" time="0">${inner}</testcase>`;
    });
    const count = (k) => cases.filter((c) => c[2] === k).length;
    suites.push(`<testsuite name="${xml(id)}" tests="${cases.length}" failures="${count('FAILED')}" errors="0" skipped="${count('ignored')}">\n${body.join('\n')}\n</testsuite>`);
  }
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, `<?xml version="1.0" encoding="UTF-8"?>\n<testsuites>\n${suites.join('\n')}\n</testsuites>\n`);
  return failed ? 1 : 0;
}

const nextestExe = nextest();
const chromium = ['install', 'chromium', ...(process.platform === 'linux' && process.env.CI ? ['--with-deps'] : [])];
run(process.execPath, [join(root, 'node_modules', 'playwright', 'cli.js'), ...chromium]);

const host = join(root, 'target', 'debug', win ? 'vto-contract-host.exe' : 'vto-contract-host');
rmSync(host, { force: true });
const built = run('cargo', ['build', '--workspace', '--bin', 'vto-contract-host']) === 0 && existsSync(host);
if (!built) console.error('vto-contract-host was not built: every backend contract test will fail.');

const rustXml = join(root, 'target', 'nextest', 'ci', 'junit.xml');
const webXml = join(root, 'target', 'vitest', 'junit.xml');
rmSync(rustXml, { force: true });
rmSync(webXml, { force: true });
const hostEnv = { VTO_CONTRACT_HOST: built ? host : '' };
const rust = nextestExe
  ? run(nextestExe, ['nextest', 'run', '--workspace', '--profile', 'ci', '--no-fail-fast'], hostEnv)
  : runWithoutNextest(hostEnv, rustXml);
const web = run(process.execPath, [join(root, 'node_modules', 'vitest', 'vitest.mjs'), 'run']);

let missing = false;
const suites = [];
for (const [xml, rustFiles] of [[rustXml, true], [webXml, false]]) {
  if (!existsSync(xml)) {
    console.error(`no report at ${xml}`);
    missing = true;
    continue;
  }
  const text = readFileSync(xml, 'utf8').replace(/<\?xml[^>]*\?>/, '');
  let body = text.match(/<testsuites\b[^>]*>([\s\S]*)<\/testsuites>/)?.[1] ?? text;
  if (rustFiles) {
    // nextest names a suite `<package>::<test binary>`; the binary is tests/<binary>.rs.
    body = body.replace(/<testsuite\b[^>]*\bname="([^"]*)"[\s\S]*?<\/testsuite>/g, (suite, name) => {
      const file = `contract-tests/tests/${name.split('::').pop()}.rs`;
      if (!name.includes('::') || !existsSync(join(root, file))) return suite;
      return suite.replace(/<testcase\b(?![^>]*\bfile=)/g, `<testcase file="${file}"`);
    });
  }
  suites.push(body.trim());
}
mkdirSync(dirname(resolve(report)), { recursive: true });
writeFileSync(report, `<?xml version="1.0" encoding="UTF-8"?>\n<testsuites>\n${suites.join('\n')}\n</testsuites>\n`);
process.exit(rust === 0 && web === 0 && built && !missing ? 0 : 1);
