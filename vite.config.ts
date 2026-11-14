// The app's screens (S1 `/`, S2 `/preferences`): Svelte and Tailwind, built to
// dist/ for the Tauri window (src-tauri/tauri.conf.json) and for `vto-web`.
// vitest.config.ts loads this file too, so the overlay tests run with the same plugins.
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';
import { execFileSync } from 'node:child_process';

function revision() {
  try { return execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim(); }
  catch { return 'unknown'; }
}

export default defineConfig({
  plugins: [svelte(), tailwindcss(), {
    name: 'build-revision',
    transformIndexHtml: () => [{ tag: 'meta', attrs: {
      name: 'wiki-build-head', content: revision(),
    }, injectTo: 'head' }],
  }],
  build: { outDir: 'dist', emptyOutDir: true },
});
