// Bundles the overlay into the single script the app injects into site pages.
// Run by src-tauri/build.rs for the `app` feature; output goes to $VTO_OVERLAY_OUT.
import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    lib: { entry: 'src/overlay/inject.ts', formats: ['iife'], name: 'vtoOverlay', fileName: () => 'overlay.js' },
    outDir: process.env.VTO_OVERLAY_OUT ?? 'target/overlay',
    emptyOutDir: true,
    minify: true,
  },
});
