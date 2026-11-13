# Library acceptance evidence

The task uses the existing device-local storage and browser. Acceptance data was
isolated from the installed user's library. No real transcription or paid Gemini
translation was requested for acceptance.

## Focused checks

| Command | Observed result |
| --- | --- |
| `cargo test -p vto-contract-tests --test library --test jobs --test settings` with `VTO_CONTRACT_HOST` pointing to the rebuilt host | 4 library, 9 job, and 3 settings tests passed. Includes restart, zero additional transcription/Gemini calls, unchanged cost, active deletion refusal, multiple memberships, confirmation cancellation, cleanup failure/retry, and unrelated-data preservation. |
| `cargo test -p vto-contract-tests --test browser -- --nocapture` | 7 passed. One earlier sign-in restart assertion failed once; its individual case and the full file then passed. The assertion now includes the host logs if it fails again; no cause is claimed. |
| `cargo test -p video-translate-overlay --lib api::` | 9 passed, including background-service admission and refreshed YouTube source policy. |
| `cargo test -p video-translate-overlay --lib commands::` | 3 passed, including local-only library permissions and Android remote-overlay restrictions. |
| `cargo test -p video-translate-overlay --lib store::` | 4 passed. A database with the pre-library schema retains exact cue times and cost; confirmed deletion removes all owned table rows and keeps the playlist. |
| `npx vitest run -c vitest.ui.config.ts tests/ui/library.test.ts tests/ui/watch.test.ts tests/ui/subtitles.test.svelte.ts` | 22 passed in Chromium. Includes named confirmation/cancellation, retry, navigation ownership, disabled active-video deletion, and rejection of late subtitles. |
| `npx vitest run tests/overlay/overlay.test.ts` | 13 passed. Includes ready/failed stale-response rejection, open-overlay deletion, seek/pause timing, advertising, and screen fit. |
| `npm run build` | Passed with the existing UI toolchain. |

These are focused checks, not the repository gate. The server owns the full gate
and final hosted verification for this public repository with active CI. They
remain pending until the server runs them.

## Actual application flows

The browser used the real `vto-web` HTTP API and Chromium. Windows used the native
Tauri executable and WebView2, including a complete process restart. Android used
the installed debug APK on the API 34 x86-64 emulator `vto34`, under the separate
Android user `VtoLibraryAcceptance` (ID 10), including force-stop/relaunch and an
APK update. This is installed-app acceptance, not a mobile viewport substitute.
There was no physical Galaxy S21 run, installer distribution check, or new
speech-performance measurement.

Each isolated fixture began with the original schema from `dc6fec7`, two ready
videos, saved original/translated cues, glossaries, and a recorded cost of
12.5 KRW per video. Actual UI actions created, renamed, and reordered playlists;
added saved videos and an untranslated link; reordered and removed entries;
reopened saved subtitles; cancelled deletion; and confirmed removal from two
playlists. After restart, order and membership persisted. The unrelated saved
translation and playlists remained. Deleting a playlist retained the translation.
An old `vto-open:fixture-saved` activation was refused on both native platforms
after the saved video was deleted.

Screenshots and machine-readable snapshots are in the ignored local folder
`artifacts/library-acceptance/`. The local acceptance scripts use actual browser
actions and native WebView debugging; they are fixture tools, not production
commands. `flow.mjs <browser|windows|android> organize` and `verify` record the
two phases around a real process restart. `smoke.mjs` checks the final native
bundles' saved cues/cost and explicit opening of an untranslated local video.

The Android media navigation emitted seven Tauri initialization/plugin diagnostics
(immutable-property redefinition and notification-plugin permission lookup).
Opening the same saved media through the existing address bar produced the exact
same diagnostics and displayed the saved subtitle. The library acceptance checks
compare that control flow rather than claiming a clean Android console. Native
notification delivery and background processing remain covered by the existing
implementation and focused job tests; this task does not claim a new physical
device notification or long-running speech test.

## Screen and test cleanup audit

After the last UI change, the library was measured at 1280 and 384 CSS pixels,
with 44 px controls and no horizontal overflow; 200% zoom and RTL also fit. Visual
inspection caught and fixed a phone rename field that had shrunk too far. The
approved C1 colors and existing typography are unchanged.

The new storage/API, library UI, and stale-response tests were audited together
with the existing API, command, browser, background-job, storage, timing, and
subtitle checks they exercise. No obsolete tests, duplicate intermediate cases,
or expired skips remain from this task. The stale-response test covers both ready
and failed jobs in one parameterized check. Existing manual native-toast skips
still require their stated desktop interaction and were retained.
