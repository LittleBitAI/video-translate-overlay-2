# Library acceptance evidence

The task uses the existing device-local storage and browser. Acceptance data was
isolated from the installed user's library. No real transcription or paid Gemini
translation was requested for acceptance.

## Production verification at 682b275

The latest production evidence covers `682b275d9dc20794ea7b9f283509da0cc0a5e68a`.
The focused checks below ran against the final sources committed in that repair.
Results from earlier commits are identified separately below. A documentation-only
descendant can reuse these results while its application and test sources remain
unchanged; this record does not claim a new execution or a newly stamped binary.

| Command | Observed result |
| --- | --- |
| `cargo test -p vto-contract-tests --test library`, with `VTO_CONTRACT_HOST` pointing to the host rebuilt by `cargo build -p video-translate-overlay --bin vto-contract-host` | 5 passed. Includes deletion confirmation/retry, active-job refusal, reuse without additional transcription or translation, playlist persistence, and clearing only matching open video URLs. |
| `cargo test -p video-translate-overlay --lib store::` | 5 passed. Includes a four-case deletion/navigation interleaving through two SQLite connections, legacy cue/cost preservation, late sign-in ownership, atomic region saving and concurrent reads. |
| `npx vitest run -c vitest.ui.config.ts tests/ui/library.test.ts tests/ui/watch.test.ts tests/ui/subtitles.test.svelte.ts` | 45 passed in Chromium. The library file expands to 13 cases, including eight two-view notification cases across native events and BroadcastChannel, successful/rejected/unreachable mutations and failed opening. Queued refreshes remove deleted rows and saved availability while retaining errors. |

The test cleanup audit for this repair retained all current checks and removed
none. The storage interleaving and API URL-selection tables guard different
boundaries. The screen cases cover both busy operation paths and both deletion
notification transports.

Overlay code and tests were unchanged between `3e31416745dda21cbd35132c84fd4c2c1dab37e6`
and `682b275`. The retained check at `3e31416` was
`npx vitest run tests/overlay/hidden_mount.test.ts tests/overlay/overlay.test.ts`:
33 passed, including hidden-page return, deletion refresh, recreation, seek/pause
and advertisement behavior. This check was not rerun for `682b275`.

### Runtime evidence at 682b275

Four fresh flows passed 22 assertions: API 4, Chromium browser 6, installed
Windows 6 and installed Android 6. Both the running cores and screens identified
`682b275`. Reopened subtitles retained the complete response, intervals
`[[0,2000],[2000,5000]]` and cost `12.5` KRW. Actual actions exercised process
restart, deletion cancellation, confirmed deletion from two playlists, unrelated
translation preservation, playlist removal and manual untranslated-link opening.
No new transcription or Gemini translation was requested; all owned native
profiles were removed.

Windows used WebView2 `157.0.4341.46`; the executable SHA-256 was
`a30a98a5efd20296cf4362c7e6d5caa5dac9a5f1f78d0909d20ea190dd7280d4`.
Android used an installed debug APK on a disposable API 34 x86-64 emulator with
WebView `113.0.5672.136`; the APK SHA-256 was
`e9e9ce7999e3e2c4467640e1591e1e367b8a3e2b56b0a9548efe97b6b0d76533`.

These were manual diagnostics using an isolated copy of wiki-agent collector
`591f408`. The restarted host at `063bddc` had no implemented WebView collector;
these receipts are not a claim that its registered collection passed. No task
pass records were written. Local receipts are retained under the ignored
`artifacts/` tooling and temporary attempt directories; the observations and build
identities above describe the evidence without requiring those local files.

The actual `node scripts/live.mjs --stand-ins` command also served matching
backend/screens at `682b275`, exposed two saved videos, and closed its owned
process tree. The full gate and final verification remain server-owned; these
focused results do not assert that the gate passed. There was no physical S21,
installer-distribution, paid-translation or new speech-performance run.

## Historical baseline at 4b3db1e

The following checks, application observations and visual audit belong to the
original feature sources committed as `4b3db1edddb036e4314ff544696b411353958e70`.
Their counts and environments describe that baseline, not `682b275` or later
repairs. They are retained as historical evidence.

### Focused checks

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

These were focused baseline checks, not a passing repository gate. Gate status
belongs to the server's verification record for each candidate commit.

### Actual application flows

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

### Screen and test cleanup audit

Before the original feature commit, the library was measured at 1280 and 384 CSS pixels,
with 44 px controls and no horizontal overflow; 200% zoom and RTL also fit. Visual
inspection caught and fixed a phone rename field that had shrunk too far. The
approved C1 colors and existing typography are unchanged.

The new storage/API, library UI, and stale-response tests were audited together
with the existing API, command, browser, background-job, storage, timing, and
subtitle checks they exercise. No obsolete tests, duplicate intermediate cases,
or expired skips remain from this task. The stale-response test covers both ready
and failed jobs in one parameterized check. Existing manual native-toast skips
still require their stated desktop interaction and were retained.
