import { loadConfigFromFile, mergeConfig } from 'vite';
import { defineConfig } from 'vitest/config';
import { playwright } from '@vitest/browser-playwright';

// The app's own vite.config.* (Svelte, Tailwind plugins) also builds the overlay
// under test. Its `test` section is ignored: what runs is decided here.
const app = await loadConfigFromFile({ command: 'serve', mode: 'test' }, undefined, import.meta.dirname);
const { test: _ignored, ...base } = app?.config ?? {};

export default mergeConfig(
  base,
  defineConfig({
    test: {
      include: ['tests/overlay/**/*.test.ts'],
      testTimeout: 60_000,
      browser: {
        enabled: true,
        headless: true,
        provider: playwright(),
        instances: [{ browser: 'chromium' }],
      },
      reporters: ['default', ['junit', { addFileAttribute: true }]],
      outputFile: { junit: 'target/vitest/junit.xml' },
    },
  }),
);
