// Builds the screens and `vto-web`, then runs it: `PORT=8080 APP_DATA_DIR=./data node scripts/live.mjs`.
// It serves the screens (S1 `/`, S2 `/preferences`) and the API on 127.0.0.1:$PORT and keeps
// all its data under $APP_DATA_DIR. With LIBCLANG_PATH set it is built with the on-device
// model (the `whisper` feature); without it a job stops at the writing stage.
import { spawn, spawnSync } from 'node:child_process';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '..', '..');
for (const name of ['PORT', 'APP_DATA_DIR']) {
  if (!process.env[name]) {
    console.error(`${name} is not set`);
    process.exit(2);
  }
}
const step = (cmd, args) => {
  const r = spawnSync(cmd, args, { cwd: root, stdio: 'inherit' });
  if (r.status !== 0) process.exit(r.status ?? 1);
};
step(process.execPath, [join(root, 'node_modules', 'vite', 'bin', 'vite.js'), 'build', '--logLevel', 'warn']);
const features = process.env.LIBCLANG_PATH ? ['--features', 'whisper'] : [];
step('cargo', ['build', '-p', 'video-translate-overlay', '--bin', 'vto-web', ...features]);

const exe = join(root, 'target', 'debug', process.platform === 'win32' ? 'vto-web.exe' : 'vto-web');
const app = spawn(exe, [], { cwd: root, stdio: 'inherit', env: { ...process.env, VTO_DIST: join(root, 'dist') } });
for (const sig of ['SIGINT', 'SIGTERM']) process.on(sig, () => app.kill(sig));
app.on('exit', (code, signal) => process.exit(code ?? (signal ? 1 : 0)));
