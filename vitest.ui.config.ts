// The screens' tests (`npm run test:ui`), in Chromium like the overlay's.
// vitest.config.ts is the pinned contract suite and runs only tests/overlay.
import { playwright } from '@vitest/browser-playwright';
import { defineConfig, mergeConfig } from 'vitest/config';
import app from './vite.config';

export default mergeConfig(
  app,
  defineConfig({
    test: {
      include: ['tests/ui/**/*.test.ts', 'tests/ui/**/*.test.svelte.ts'],
      testTimeout: 30_000,
      browser: {
        enabled: true,
        headless: true,
        provider: playwright(),
        instances: [{ browser: 'chromium' }],
      },
    },
  }),
);
