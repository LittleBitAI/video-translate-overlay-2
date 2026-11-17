# Maximized Windows layout evidence

Final application and test sources: `5c83f9a61d98fd225c2117f89abe47b6556af896`.
The evidence-only descendant does not change build inputs. User acceptance is
pending; these focused checks do not claim the server-owned repository gate.

## Installed baseline

The installed Windows app was version 0.1.8 at
`%LOCALAPPDATA%/video-translate-overlay/video-translate-overlay.exe`, SHA-256
`2cc473e758178127b7f28ce55e3f2587bd8fe65672f300f949ce4e3f74f504df`.
Actual Windows UI Automation navigation and maximize/restore reproduced the
excessive margins on Video, Library and Settings before implementation. Its
viewport changed from 1024×760 to 1920×1017 CSS pixels; the content stayed
approximately 1024, 1152 and 672 px wide respectively. The menu and content
also used different leading edges. `baseline/windows.json` retains the window
and UI Automation geometry. Installed-user screenshots stay in the ignored
`artifacts/maximized-layout/installed-baseline/` directory because they include
the existing account and library. Baseline actions changed only navigation and
window state, then returned the window to its original state and Library.

`baseline/browser.json` measures the starting screens at all four requested
viewports using an isolated stand-in server and 30 untranslated fixture links.

## Visual revisions

1. `iterations/rejected-cards.png`: the initial two-column Windows Settings
   rendering at `1540dd6`. The user rejected its oversized cards, uneven groups
   and disconnected whitespace. This is not accepted final evidence.
2. `iterations/flat-first.png`: the first flat browser revision. Direct visual
   inspection found cramped 256 px descriptions and separators extending far
   beyond their content. `flat-first.json` retains its measurements.
3. The final flat rows widen descriptions to 384 px, cap controls at 672 px,
   shorten separators to the readable content, and compact the key-removal
   warning/action. `flat-refined.json` records 1280, 1920, 2560, 768 and 384 px
   inspections. At 384 px every Settings section retains the baseline position
   and height. These intermediate renders used the working tree before the final
   source commit; the final captures below identify the committed build.

The existing C1 palette, 14/16/20/28 px type scale and screen navigation remain.
Desktop grouping uses shared edges, whitespace and quiet separators. The phone
keeps its existing cards and section order. No content was added to fill space.

## Final browser measurements

`node scripts/live.mjs --stand-ins` served the final screens and Rust API with
`PORT=8145` and the isolated `artifacts/maximized-layout/baseline-data` profile.
`node artifacts/maximized-layout/browser.mjs` clicked the shared navigation,
exercised wheel scrolling and the library's edit/add toggles, measured the DOM
and captured each screen. Both the API build and screen metadata identified
`5c83f9a`. Measurements and screenshots are in `browser/`.

| CSS viewport | Desktop leading edge | Video stage | Library columns | Settings controls | Horizontal overflow |
| --- | --- | --- | --- | --- | --- |
| 1280×720 | 16 px | 1109×624 | 4 | x 424, width 672 | none |
| 1920×1080 | 16 px | 1749×984 | 7 | x 424, width 672 | none |
| 2560×1440 | 16 px | 2389×1344 | 10 | x 424, width 672 | none |
| 384×854 | existing phone insets | 368×207 | 1, accordion | existing 336 px inner width | none |

The desktop player remains 16:9 and ends within the viewport. Settings labels
share the page's 16 px edge; all four control groups share their second edge.
The bottom phone menu remains at y 809 with its existing 45 px height.

`node artifacts/maximized-layout/settings-confirmation.mjs` additionally opened
the real key-removal confirmation at 1280, 768 and 384 px against a dummy key
on the stand-in server. Controls fit without horizontal overflow; cancelling
preserved the key. The temporary key was then cleared from that test profile.
The confirmation screenshots and measurements are in `browser/`.

## Final Windows measurements

Windows 11 Home build 26200, WebView2 157.0.4341.46, Playwright 1.61.0.
`cargo build -p video-translate-overlay --features app --bin video-translate-overlay`
passed with the repository's documented `LIBCLANG_PATH`. The initial attempt
without that environment setting failed to find libclang; the configured build
then passed. The final executable SHA-256 is
`a086c6a734958a8dbd5b07d456fdf7acfe691e3db84fca6aee020f09512c9872`.

`python artifacts/maximized-layout/launch-windows.py` launched that executable
in the host's owned process context with fresh application and WebView2 profiles.
`node artifacts/maximized-layout/windows.mjs` verified the core and screen heads,
the profile digest and ownership of the debugging listener. It created fixture
playlists through the app's real IPC and exercised:

| Actual Windows state | CSS viewport | Library columns | Settings control width | Result |
| --- | --- | --- | --- | --- |
| Restored | 1024×760 | 3 | 569 px | navigation, scrolling and layout passed |
| Maximized with native WindowPattern | 1920×1017 | 7 | 672 px | navigation, scrolling and layout passed |
| Restored again | 1024×760 | 3 | 569 px | navigation, scrolling and layout passed |

`windows/measurements.json` records native state, window geometry, process and
listener identity, build identity, DOM measurements and scroll results. Every
state has a rendered capture of all three screens. The maximized Settings
rendering was inspected directly; the restored page scrolls to its remaining
controls. No clipping, overlap, horizontal overflow or navigation change was
observed. These are focused manual native checks, not registered-flow receipts.
No new transcription, paid translation, account sign-in or installer upgrade
was requested. The installed user's app was not replaced by the debug preview.

## Focused checks and test cleanup

`npx vitest run -c vitest.ui.config.ts tests/ui/library-layout.test.ts tests/ui/preferences.test.ts tests/ui/watch.test.ts tests/ui/library.test.ts tests/ui/google-account.test.ts`
passed: 5 files, 67 tests. `npm run build` passed after the final code change;
the native and live launch preparations rebuilt the same committed screens.
The complete repository gate and final verification remain server-owned.

The cleanup audit covered all eight UI test files. None tests a removed path,
obsolete format, superseded intermediate implementation or duplicate observation;
none was removed. The existing layout table now covers the larger viewports and
flat Settings rows. The scroll regression retains its original assertions and
uses 36 entries so the wider grid still produces a genuinely scrolled page.
State, account, deletion, subtitle and update checks continue to guard current
behavior. No whole-directory pytest or complete repository gate was run here.

`sha256.json` pins the committed screenshots, measurements and focused logs;
text hashes use the repository's LF line endings.
