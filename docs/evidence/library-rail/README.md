# Library rail, accordion and shared menu: captures

Real renderings of the library change, for the pull request and the design
record (`docs/design/frontend.md`, S3 and "Design pass, 2026-11-17").

## What was captured

| Folder | Source | Revision | Viewports |
| --- | --- | --- | --- |
| `browser/before-*` | `node scripts/live.mjs --stand-ins` in headless Chromium | base `237caab` | 1280×720, 384×854 |
| `browser/after-*` | the same stand-in server and data | `f20eef0` | 1280×720, 384×854 |
| `windows/` | the debug Windows app (`target/debug/video-translate-overlay.exe`), WebView2 | `f20eef0` | its own 1,024×760 viewport (1,009 px page width) |
| `android/` | the x86_64 debug APK on the API 34 emulator (`vto34`, booted read-only) | `f20eef0` | 412×842 at 2.625 device pixels per CSS pixel |

The browser data is five YouTube lectures, one link from another site
(`lectures.example.org`, which gets the placeholder tile) and three playlists:
딥러닝 기초 (four lectures), LLM 강연 모음 (two) and the empty 나중에 볼 것. The
native apps were seeded with `scripts/verification/seed.py` (two saved
translations served from `127.0.0.1:8137`, which also get the placeholder) and
the same playlists through the apps' own library commands.

Each folder holds the library with a playlist selected (`library-playlist`),
edit mode (`library-edit`), the add panel (`library-add`), an empty playlist
(`library-empty-playlist`), the watch screen (`watch`) and settings
(`settings`). `browser/` also has the library as first opened (`library`), and
the before set has full-page shots of a selected playlist
(`before-library-playlist-*-full.png`) showing how far down its videos began.
No before captures exist for the native apps; the browser before set shows the
same screens, since both apps load the same built screens.

## Measurements

`*-measured.json` hold the first video card's box after a playlist is
selected, and the page's scroll width:

| Capture | First card | Page width / viewport |
| --- | --- | --- |
| before, 1280×720 | y 1,237 (below the fold) | 1,280 / 1,280 |
| before, 384×854 | y 1,483 (below the fold) | 384 / 384 |
| after, 1280×720 | x 352.5, y 137 | 1,265 / 1,280 (15 px scrollbar gutter) |
| after, 384×854 | x 16, y 266 | 384 / 384 |
| Windows app | x 296, y 137 | 1,009 / 1,024 |
| Android emulator | x 16, y 266 | 412 / 412 |

`design-pass-measure.json` is the raw output of design-pass step 2 at
1280×720, 1009×720 and 384×854: padding, positions and widths of the menu,
heading, list items, toolbar and cards, same-role disagreements, and the
state each of 편집, 완료, 추가 and 닫기 produced when pressed.

## Findings from the native renderings

Both were fixed before these captures; the design record describes them.

- Windows: at the app's width a grid card is about 210 px wide, so in edit mode
  its address broke mid-word and its buttons wrapped onto four lines. Edit
  mode became a one-column list, and the wide layout reserves the scrollbar
  gutter.
- Android: the WebView (Chrome 113) drew Tailwind's default placeholder colour
  as transparent, leaving the search field blank. The library's inputs now set
  their placeholder colour explicitly.

## Runtime receipts at `27ca194`

`receipts/` holds the four registered library flows, run at
`27ca19436155d401ed567f65bc27f149d141907b` (the head after the review-round
fixes for move clamping and list reveal) with a clean tree. Each receipt
names that head; nothing in it is reused from an earlier run.

| File | Flow | What it records |
| --- | --- | --- |
| `api.json` | `library-api`, `node scripts/verification/run.mjs api` | 26 real requests to the runner's own `vto-web --stand-ins` on port 8136 (method, URL, status) and four observations, each with expected and actual values: reuse, persistence-delete, no-automatic-translation, playlist-retention |
| `browser.json`, `browser/` | `library-browser`, `node scripts/verification/run.mjs browser` | build head `27ca194` reported by the server's `GET /build`, browser tool `playwright-chromium`, 68 requests, six actions and six observations with expected and actual values, and a screenshot plus data snapshot per step |
| `windows/` | `library-windows`, wiki-agent's owned WebView driver | `result.json` with build head `27ca194`, the binary's SHA-256, the driver `owned-webview-v1`, the environment revisions (Windows 11 26200, Playwright 1.61.0), eight native sessions (process, listener, window geometry and DPI at launch and at each assertion) and six observations; every listed artifact is copied here and matches its recorded SHA-256 |
| `android/` | `library-android`, the same driver on a disposable API 34 x86-64 emulator | the same fields, plus emulator process, build fingerprint digest, focus records and a device screenshot per checkpoint |

All four passed: 4 of 4 API observations and 6 of 6 for each of browser,
Windows and Android.

Two things about how the native flows ran:

- Approval. The local approval file (ignored by Git) still names binaries
  from older builds and an AVD name the current driver refuses. Each run
  built its request from an in-memory copy with the freshly measured binary
  hashes and the system image's digest; `*/approval-used.json` lists both
  sets of hashes. The stored file was not changed.
- A blocked Android attempt. The first Android run at this head stopped
  during launch: after the app's process, devtools socket and forward were
  found, the driver's next process check returned nothing
  (`android-blocked-attempt.json`). The app's own log showed it running at
  that moment. The single rerun on the same build passed and is the receipt
  in `android/`.

Reuse of the captures. The captures above were taken at `f20eef0`. Between
it and `27ca194` the screens changed only in what happens after a playlist
is activated (the page returns to the list's start); the store change
saturates large move values. Neither changes how any captured state looks.
The commit that adds these receipts changes only files under
`docs/evidence/`, none of which is a build input of any flow.
