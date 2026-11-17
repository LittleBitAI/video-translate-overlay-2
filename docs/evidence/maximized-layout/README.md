# Maximized Windows layout evidence

Candidate application and test sources: `d9d1df6f081a158bbb2adf561eed3758d77f60ad`.
Evidence-only descendants do not change build inputs. User acceptance is pending.
The earlier card, flat-row and capped hierarchy candidates were rejected; none is accepted
design evidence. These focused checks do not claim the server-owned repository gate.

## Installed baseline

The installed app was Windows 0.1.8 at
`%LOCALAPPDATA%/video-translate-overlay/video-translate-overlay.exe`, SHA-256
`2cc473e758178127b7f28ce55e3f2587bd8fe65672f300f949ce4e3f74f504df`.
Actual native navigation and maximize/restore reproduced excessive margins on
Video, Library and Settings before implementation. Its viewport changed from
1024×760 to 1920×1017 CSS pixels; content stayed approximately 1024, 1152 and
672 px wide respectively. The menu and content also used different leading
edges. `baseline/windows.json` retains the UI Automation geometry.

Installed-user screenshots stay in the ignored
`artifacts/maximized-layout/installed-baseline/` directory because they contain
the existing account and library. Actions changed navigation and window state,
then returned the window to its original state and Library.
`baseline/browser.json` records the starting browser layouts at all four sizes.

## Visual revisions and current hierarchy

- `iterations/rejected-cards.png`: the initial Windows card layout at
  `1540dd6`, rejected for oversized boxes, uneven groups and disconnected space.
- `iterations/flat-first.png` and `flat-first.json`: the first flat revision.
  Inspection found cramped 256 px descriptions and excessively long separators.
- `iterations/rejected-flat-rows.png` and `flat-refined.json`: the wider flat
  rows at `5c83f9a`. The user rejected this revision too: privacy appeared before
  setup, the task hierarchy remained wrong, and the grouping was unsatisfactory.
- `iterations/hierarchy-first.png` and `hierarchy-first.json`: setup first,
  accounts together and privacy in the footer. Inspection at 2560 px found the
  fluid columns and account text too far apart.
- `hierarchy-refined.json` and `rejected-capped-hierarchy.png`: the 1440 px
  capped workspace at `700c30e`. The user rejected its remaining right margin,
  unequal Gemini/YouTube bottom actions and subdued update control.
- `reconstruction-first.json`: a fresh composition measured at 1920×1080 first,
  then at 1280, 2560 and 384 px. The committed candidate is in `browser/` and
  `windows/`. Revision-specific copies are
  `browser/settings-1920x1080-d9d1df6.png` and
  `windows/settings-maximized-d9d1df6.png`.

The current desktop candidate uses the available page width without an overall
cap. Gemini setup and YouTube session controls occupy equal-height grid cells
and share their top and bottom edges. Their bottom action rows align through
flex layout, without fixed-height spacers. The key form and removal group can
grow to 960 px. Google app-account sync occupies the next full-width row, with
its description and status following the same two column edges. Update checking
is a prominent outlined blue 44 px button in the top-right header. Privacy stays
in the footer, available before sign-in with the same native opener and retry.
There are no desktop card backgrounds or rounded section boxes.

At narrow widths the same task order stacks, with the existing controls and
confirmations. This hierarchy correction intentionally changes the earlier
Settings order; the shared phone navigation and library accordion are unchanged.
The approved C1 palette and existing type sizes remain. No filler content was added.

## Current browser measurements

`node scripts/live.mjs --stand-ins` served the committed screens and Rust API
with `PORT=8145` and the isolated
`artifacts/maximized-layout/baseline-data` profile. Thirty fixture lecture links
were added without translation jobs.
`node artifacts/maximized-layout/browser.mjs` clicked navigation, exercised
wheel scrolling and library edit/add toggles, measured the DOM and captured
all three screens. Both core and screen metadata identified `d9d1df6`.

| CSS viewport | Leading edge | Video stage | Library columns | Key form width | Horizontal overflow |
| --- | --- | --- | --- | --- | --- |
| 1280×720 | 16 px | 1109×624 | 4 | 604.5 px | none |
| 1920×1080 | 16 px | 1749×984 | 7 | 924.5 px | none |
| 2560×1440 | 16 px | 2389×1344 | 10 | 960 px | none |
| 384×854 | existing phone insets | 368×207 | 1, accordion | 336 px | none |

The player remains 16:9 and ends within the viewport. The phone menu remains
at y 809 with its existing 45 px height. `browser/` holds the current screenshots
and measurements, including footer placement and the order of Settings groups.

At 1920×1080 the workspace ends at x 1889: the remaining 31 px is the 16 px
page inset plus the reserved 15 px scrollbar gutter. Both key removal and
YouTube logout end at y 421.39. The update control is at y 77.19, 44 px high,
with its right edge aligned to x 1889. The 1280 and 2560 px checks also verify
equal action bottoms, the same right inset and a visible header update control.

`node artifacts/maximized-layout/settings-confirmation.mjs` opened the actual
key-removal confirmation at 1280, 768 and 384 px against a dummy key on the
stand-in server. The dialog and controls fit; cancelling preserved the key.
The temporary key was cleared from that test profile afterward.

## Current Windows evidence

Windows 11 Home build 26200, WebView2 157.0.4341.46, Playwright 1.61.0.
`cargo build -p video-translate-overlay --features app --bin video-translate-overlay`
passed with the repository's documented `LIBCLANG_PATH`. An initial earlier
attempt without that setting failed to find libclang; the configured builds
passed. The current executable SHA-256 is
`ef1f02cf32d28475570053ec91048a6130ead394a9f2258eb552750b78dd728c`.

`python artifacts/maximized-layout/launch-windows.py` launched the executable
in the host's owned process context with fresh application and WebView2 profiles.
`node artifacts/maximized-layout/windows.mjs` verified the core and screen heads,
profile digest and ownership of the debugging listener, seeded playlists through
real IPC and exercised all three screens through actual native window changes.

| Native state | CSS viewport | Library columns | Key form width | Settings footer |
| --- | --- | --- | --- | --- |
| Restored | 1024×760 | 3 | 476.5 px | y 681–744 |
| Maximized with native WindowPattern | 1920×1017 | 7 | 924.5 px | y 938–1001 |
| Restored again | 1024×760 | 3 | 476.5 px | y 681–744 |

Native Gemini/YouTube action bottoms match at y 442.39 in both restored runs
and y 421.39 while maximized. The update button stays 44 px high at y 77.19;
the workspace retains the 16 px right inset in all three states.

Navigation, wheel scrolling and layout checks passed in every state. No clipping,
overlap, horizontal overflow or navigation change was observed. The current
maximized rendering was inspected directly after rebuilding the native app.
`windows/measurements.json` records process/listener ownership, native window
state and geometry, build identity and measured controls; screenshots cover
every state and screen. These are focused manual checks, not registered-flow receipts.

No new transcription, paid translation, account sign-in or installer upgrade was
requested. The installed app was not replaced by the isolated debug preview.

## Focused checks and test cleanup

`npx vitest run -c vitest.ui.config.ts tests/ui/library-layout.test.ts tests/ui/preferences.test.ts tests/ui/watch.test.ts tests/ui/library.test.ts tests/ui/google-account.test.ts tests/ui/updates.test.ts`
passed: 6 files, 76 tests. `npm run build` passed after the last code change;
the native and live launch preparations rebuilt the committed screens.
Complete repository verification remains server-owned and pending.

The cleanup audit covered all eight UI test files. No obsolete, duplicate or
superseded test was found; none was removed. The existing layout table now
checks full-width layout, exact Gemini/YouTube bottom alignment, prominent header
update checking, footer privacy, frameless desktop groups and stacked narrow
layout. Assertions for the rejected width caps and old hierarchy were replaced
in that table. The scroll regression keeps
its assertions and uses 36 entries so the wider grid produces a scrolled page.
Privacy opening/failure/retry, update checks, account state, key confirmation,
deletion and subtitle tests still guard current behavior. No whole-directory
pytest or full repository gate was run here.

`sha256.json` pins committed screenshots, measurements and focused logs.
Text hashes use the repository's LF line endings.
