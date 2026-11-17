# Maximized Windows layout evidence

Accepted Settings reconstruction: `d9d1df6f081a158bbb2adf561eed3758d77f60ad`.
The review repair and its fresh, build-bound receipts are recorded below under
"Review round 1". On 2026-11-17 the user
selected "Accept reconstruction (Recommended)" in response to the new
1920×1080 capture, the matching action-bottom measurements, the full-width
workspace and the prominent header update button. The accepted capture is
`browser/settings-1920x1080-d9d1df6.png`; the matching actual Windows rendering
is `windows/settings-maximized-d9d1df6.png`.
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

The accepted desktop reconstruction uses the available page width without an overall
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

## Accepted browser measurements before the review repair

`node scripts/live.mjs --stand-ins` served the committed screens and Rust API
with `PORT=8145` and the isolated
`artifacts/maximized-layout/baseline-data` profile. Thirty fixture lecture links
were added without translation jobs.
`node artifacts/maximized-layout/browser.mjs` clicked navigation, exercised
wheel scrolling and library edit/add toggles, measured the DOM and captured
all three screens. These historical browser files identify screen `d9d1df6`,
but their core and profile fields are null. They do not establish the serving
core's identity. The earlier claim that both identities were recorded was wrong;
the fresh receipts below replace that claim.

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

## Accepted Windows observations before the review repair

Windows 11 Home build 26200, WebView2 157.0.4341.46, Playwright 1.61.0.
`cargo build -p video-translate-overlay --features app --bin video-translate-overlay`
passed with the repository's documented `LIBCLANG_PATH`. An initial earlier
attempt without that setting failed to find libclang; the configured builds
passed. That run's executable SHA-256 is
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
After acceptance the owned launch context closed its Windows process tree;
the app PID and its debugging listener were gone, and the stand-in server was
also stopped. `windows/cleanup.json` records these manual cleanup checks.

## Focused checks before the review repair

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

## Review round 1

### Player regression and its other render paths

At 1280×720, the expanded address-error test failed against `355cbc7`: the media
frame ended at 729.98 px with the error, and 792.98 px with the additional ready
notice. The property is that a retained media frame stays 16:9 and ends inside
the viewport after an address is rejected, whether a ready notice is absent,
present or wraps. Six cases cover 1280, 768, 1920, 2560 and 384 px widths.

The repair measures the stage's document position, subtracts it and the bottom
padding from the viewport height, and recalculates after content or window
resizes. The observer is disconnected and its scheduled frame cancelled when
the screen retires. An initial diagnostic version updated dimensions inside
the observer and produced an undelivered-notification warning; that version
was replaced before the passing source checks and runtime runs.

There are three render sites to audit: the loading placeholder, the empty
placeholder, and the opened-video stage. The latter has media, YouTube,
other-page and native-app presentations. The shared fitting rule covers all
three sites. The existing frame-fit test now covers five browser states at
each of four viewport sizes, retaining the previous media and overlay checks.
The native presentation is checked separately in the Windows receipt.

### Inspectable producers, requests and source binding

The producers are committed in `scripts/verification/maximized-layout/`:
`browser.mjs` records real isolated HTTP requests and expected/observed
assertions; `windows.mjs` records the native API calls, window transitions and
assertions; `capture.mjs` supplies the navigation, wheel, geometry and screenshot
checks. `identity.mjs` records environment versions, the answering process,
binary hashes and the Git input inventory. `inspect.ps1` verifies that the
listener belongs to the expected process tree. `launch-windows.py` owns a new
application/WebView profile and closes its process tree; its host helper sources
are copied and hashed in the Windows packet. `native-state.ps1` performs the
actual Windows restore/maximize transitions through UI Automation.

The passing collector revision is `938285973f01e1dd89710e3a476393b17d31f556`.
Both actual cores and screens report `f5277edebf2d1b0281f404177299479f19f80c55`.
This is explicit reuse of the freshly rebuilt executables after collector-only
repairs, not a claim that they were restamped. Each receipt's `buildBinding`
contains the two revisions, exclusions and the observed empty build-input diff.
Only wiki metadata, this evidence directory and the diagnostic scripts are
excluded; application sources, tests, manifests, locks and build scripts remain
in the comparison. Each `build-inputs.json` also pins the collector's complete
tracked tree outside wiki metadata and this evidence directory, including every
producer's Git blob identity. The receipt pins that inventory's digest.

`node scripts/verification/maximized-layout/audit.mjs` independently checks the
retained assertions, exact inventory entries/digests and host-source hashes. Its
build-binding property accepts two revisions with unchanged application inputs
and refuses two revisions from before the player repair. It is a read-only check
of this task's packet, not an execution of the historical runtime flows.

`review-1/source-binding.json` records the corresponding comparisons and exact
paths for historical result reuse. The original `d9d1df6` to reviewed `355cbc7`
comparison has no changed application/test inputs. Those old browser files are
reused only as screen geometry and the user's accepted Settings rendering.
Their missing core identity is not reconstructed. The repaired player has fresh
runtime measurements. No server gate or final-verification result is claimed by
this packet; those results belong to the server owner.

| Retained output | Fresh observation |
| --- | --- |
| `review-1/browser/receipt.json` | 49 direct HTTP requests, 83 expected/observed API and browser assertions, plus the actual screen requests/responses for each viewport |
| `review-1/browser/build-inputs.json` | Collector revision, environment and tracked input blob inventory |
| `review-1/browser/*-player-messages.png` | Actual media frame with an address error and wrapped ready notice |
| `review-1/windows/receipt.json` | 39 native API calls, 46 expected/observed assertions, listener/process ownership, actual window states, geometry and screenshots |
| `review-1/windows/build-inputs.json` | Native collector revision, environment and tracked input blob inventory |
| `review-1/focused-tests.json` and `.log` | Committed source identity, environment, command, exit status and final focused test totals |
| `review-1/audit.json` and `.log` | Read-only validation of 129 retained assertions, exact producer inventories, host sources and the revision-binding property |
| `review-1/cleanup.json` | Exit of the owned native tree and release of both test listeners |

The API assertions cover a fresh empty library, playlist creation, 36 retained
links without translation jobs, media opening, rejected non-HTTP addresses
preserving the open media and null job, and reading settings. Screen observations
cover the shared menu, grid, edit/add controls, wheel scrolling, action alignment,
footer placement and the frame under messages. These are focused layout and API
diagnostics, not speech accuracy or playback performance measurements. The media
address is a local fixture path; no playable lecture or paid service is used.

### Environment and final measurements

Windows 11 Home 10.0.26200, x64; Node 24.18.0; Playwright 1.61.0;
Rust 1.98.1. Headless Chromium is 149.0.7827.55; the actual Windows application
uses WebView2 157.0.4341.46. The HTTP executable SHA-256 is
`aaf0993659777202434d77539b990e4fb7355d023edb7e7210751f98fde2f576`;
the native executable SHA-256 is
`8769ee42733588e53dcfc9d40a81da9028a438c13e8aa0abfe2c39e0c8cc87ec`.

| CSS viewport | Normal frame width × height | Frame bottom with error / both messages | Library columns |
| --- | --- | --- | --- |
| 1280×720 | 1114.66×626.98 | 703.98 / 704.00 | 4 |
| 1920×1080 | 1754.66×986.98 | 1063.98 / 1063.98 | 7 |
| 2560×1440 | 2394.66×1346.98 | 1423.98 / 1423.98 | 10 |
| 384×854 | 368×207 | 326 / 486 | 1, existing accordion |

The phone's stage wrapper also holds the status panel below the 207 px media
frame; desktop fitting does not change that behavior. The native window was
restored at 1024×760, maximized at 1920×1017 and restored at 1024×760. Its stage
ends at 731.56, 1000.98 and 731.56 px with both messages present. Settings action
bottoms still match at 442.39, 421.39 and 442.39 px, respectively. All three
screens passed navigation, scrolling and horizontal-overflow checks in each state.
The browser message-state and native maximized Settings captures were inspected
directly after collection.

### Focused checks, cleanup and limits

`npx vitest run -c vitest.ui.config.ts tests/ui/watch.test.ts tests/ui/library-layout.test.ts`
passed 54 tests in two files after the final application/test change. The retained
run identifies its committed source in `focused-tests.json`. `npm run build`
passed; the native build command above passed in 26.91 seconds. The actual
`node scripts/live.mjs --stand-ins` command rebuilt and served the matching
screens and HTTP core with port 8146 and a fresh isolated profile.

The test cleanup audit covered the eight existing UI test files. No stale,
duplicate, skipped or superseded check needed removal. The existing invalid-address
and frame-fit scenarios were extended rather than adding parallel tests for the
same observations. Address rejection, retained playback, ready-notice placement,
stage variants, navigation, account ownership, deletion and narrow-screen layout
remain guarded. Only the two affected files were rerun for this repair.

Two preliminary collector attempts stopped before mutations: unsupported Git
tree path filters, then an overly strict empty-library assertion that rejected
the legitimate account metadata. Both collector errors were diagnosed and fixed;
the passing receipts use the expected empty videos/playlists observation while
retaining the complete actual API response. No failed attempt is counted as a pass.
The installed user's app and data were not replaced. No sign-in, transcription,
paid translation, installer update or physical-phone run was performed.
