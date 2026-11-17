# Automatic fullscreen subtitle sizing

Application revision: `aedada2`. User acceptance of fullscreen readability is
pending. These are focused execution checks; the server-owned repository gate
and final hosted CI have not been run by this session.

## Player and build

The installed baseline is Windows 0.1.8 at
`%LOCALAPPDATA%/video-translate-overlay/video-translate-overlay.exe`, SHA-256
`2cc473e758178127b7f28ce55e3f2587bd8fe65672f300f949ce4e3f74f504df`.
Desktop `TauriBrowser::navigate` opens the separate `site` window. It injects
`src/overlay/inject.ts`, which mounts `src/overlay/index.ts` on the main video.
That renderer had the 26 px ceiling. The local Svelte screen instead uses
`Stage.svelte` and `Overlay.svelte`; its former viewport breakpoints did not
follow the video size.

The tested preview is a real Windows app built with:

```powershell
$env:LIBCLANG_PATH='C:/Users/dasdk/AppData/Local/Programs/Python/Python310/Lib/site-packages/clang/native'
cargo build -p video-translate-overlay --features app --bin video-translate-overlay
```

Result: `Finished dev profile [unoptimized + debuginfo] target(s) in 27.91s`.
This also bundles both the injected overlay and the Svelte screens through
Vite. The linker emitted its usual import-library warning; the build passed.
The environment was Windows 11 Home build 26200, WebView2 157.0.4341.46,
Node 24.18.0 and Playwright 1.61.0.

`receipt.json` records the native core/screen revision, binary SHA-256,
disposable profile digest, debugging listener ancestry, actual native window
geometry, and measured DOM bounds. The browser run used the same built screens
and the existing real `vto-web` core at `dc161893`; the collector verifies that
its Rust/Cargo inputs have no differences from the application revision.
The captures' hashes are in `captures.json`.

## Actual Windows playback

An isolated native profile contains synthetic saved Korean cues, seeded with the
existing verification helper. The player serves the repository's three-second
black MP4 fixture locally. The app injects its production bundle and loads the
saved cues through real Tauri IPC. There is no replacement overlay or subtitle
request handler. No translation or transcription was started, and the installed
user profile was not modified.

| Action/state | Actual viewport | Visible image | Subtitle size |
| --- | --- | --- | --- |
| Restored, short cue | 1280×800 | 1280×720 | 38.4 px |
| Fullscreen, short and long cues | 1920×1080 | 1920×1080 | 57.6 px |
| Fullscreen, letterboxed long cue | 1920×1080 | 1536×864 | 46.08 px |
| Fullscreen exit, long cue | 1280×800 | 1280×720 | 38.4 px |
| Native window resize, long cue | 960×600 | 960×540 | 28.8 px |

The app's own fullscreen button entered and exited fullscreen. Native UI
Automation resized the actual player window. A backward seek returned the short
cue; seeking forward returned the long cue. Actual playback advanced the source
clock, and pause kept it fixed. Korean wrapping and an explicit newline stayed
inside the visible image, with clearance above the native playback controls.
The subtitle response before and after every action was identical; its digest is
`6b8536b72f7e282156f9999155df69b246f7dc53ff5d9003df14f6c780f28696`.

## Local screen and phone fit

The browser run uses the production Svelte player and real local API with an
isolated stand-in profile. Only the existing synthetic saved cues are read.

| Browser viewport | Local subtitle size | Observations |
| --- | --- | --- |
| 1280×720 | 33.4392 px | Fits video; no page overflow |
| 1920×1080 | 52.6392 px | Fits video; no page overflow |
| 384×854 | 14 px | Fits video above controls; phone menu remains visible |

The first phone capture exposed the old local overlay's overlap with the seek
bar. Both renderers now reserve at least 48 px below subtitles, increasing to
10% of image height in larger players. The retained captures and receipt were
collected after this final code change.

## Focused checks and test cleanup

After the final code change, the affected checks passed:

```text
npx vitest run tests/overlay/overlay.test.ts tests/overlay/placement.test.ts tests/overlay/frames.test.ts tests/overlay/hidden_mount.test.ts tests/overlay/trusted_types.test.ts
Test Files 5 passed (5); Tests 43 passed (43)

npx vitest run -c vitest.ui.config.ts tests/ui/subtitles.test.svelte.ts tests/ui/watch.test.ts
Test Files 2 passed (2); Tests 41 passed (41)

node docs/evidence/fullscreen-subtitles/scripts/check.mjs
PASS: 7 actual Windows states and 3 browser viewports; saved cues unchanged
```

The new geometry checks exercise both renderers, including 4:3 and wide images,
short and long Korean cues, explicit newlines, wrapping, restore and resize.
The existing seek/pause, placement, hidden-page, frame and watch-screen checks
still guard current behavior. The cleanup audit found no obsolete tests,
superseded assertions, duplicate observations or stale skips to remove.

To reproduce the captures, run from the repository root with dependencies and
the native binary already built. Choose unused local ports 8157, 8159 and 9251,
and a fresh `artifacts/subtitle-scaling/windows-final` directory. Start
`node docs/evidence/fullscreen-subtitles/scripts/server.mjs`. Set
`WIKI_HOST_TOOL` to this wiki-agent installation's `tool` directory, then run
`python docs/evidence/fullscreen-subtitles/scripts/launch.py` for the isolated
native preview. Seed a separate browser profile with
`python scripts/verification/seed.py artifacts/subtitle-scaling/browser-data 127.0.0.1 8159`,
then run `target/debug/vto-web.exe --stand-ins` with `PORT=8157` and
`APP_DATA_DIR` pointing to that profile. Run the collector command above.
Creating `artifacts/subtitle-scaling/windows-final/stop` closes the owned native
process tree. Stop only the two servers started for this run.

The public repository has an active Windows CI workflow, so final verification
belongs to hosted CI under the server's control. These checks do not claim that
gate. No release installer, live YouTube session or physical Android device was
tested; no release was published.
