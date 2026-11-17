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
