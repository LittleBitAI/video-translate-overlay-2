# Release update notice acceptance

The user approved publication of the exact 0.1.7 artifacts and notes, and
[the release is public](https://github.com/LittleBitAI/video-translate-overlay-2/releases/tag/v0.1.7).
Anonymous metadata/download checks and native selection against the actual
release passed. The full repository gate remains server-owned and pending.

## Approved published artifacts

The packaging script completed both platforms from clean source commit
`f8ce38868089674d17c8a8410aaecb05d979e8fb`. Versions match at 0.1.7; Android's
version code is 1007. Outputs are in the ignored `artifacts/releases/0.1.7/`.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `video-translate-overlay-0.1.7-arm64.apk` | 24545528 | `c86c781d03ee6f19e38fe854ad37c1c54673bf0072b32d639f34cd11738bcc04` |
| `video-translate-overlay-0.1.7-windows-x64-setup.exe` | 4050199 | `6776b1041198786f721e590af921a92bb7e289f9cdb4b1c0dfb52d521a99b591` |

The published release body is `docs/releases/0.1.7.md`, SHA-256
`bd2eff3ff8409e1309f4e1b0cd9966ed481d59689a70fbae051c070ae29488ba`.
`manifest.json` records the clean source commit and notes hash. Android signature
verification reports certificate SHA-256
`fa6da2233c2062b48baf20b3562eacacce774da80f2e4bb3d20ec3a6a527ec28`, matching 0.1.6.
The APK retains package `dev.vto.overlay` and contains ARM64 libraries. Windows is
an unsigned x64 per-user NSIS installer. No new signing key was generated.

`pwsh -NoProfile -File scripts/package-release.ps1` completed with
`Prepared 0.1.7 (both)`. The final Windows installer was installed silently into
an isolated directory under `artifacts/update-acceptance/`; its payload contains
only `video-translate-overlay.exe` and `uninstall.exe`. Both local development
hosts are excluded. The production Android library and installed Windows
executable contain neither debug update override filename.

## Focused checks

| Command | Result |
| --- | --- |
| `cargo test -p video-translate-overlay --lib updates:: -- --nocapture` | 6 passed. Numeric ordering, approval/artifact filtering, bad metadata, dismissal high-water mark, manual reveal, revalidation, shared requests, failure/recovery, and the 15-second caller timeout. |
| `cargo test -p video-translate-overlay --lib commands:: -- --nocapture` | 3 passed. Routes include all new commands; remote overlays retain only their existing three permissions on Windows and Android. |
| `cargo build -p video-translate-overlay --bin vto-contract-host --bin vto-web` | Passed with default features after gating development binaries. Existing local commands still work. |
| `cargo test -p vto-contract-tests --test settings -- --nocapture` with `VTO_CONTRACT_HOST` pointing to the rebuilt host | 3 passed. Key masking/storage/deletion and remote-site denial remain intact. |
| `cargo test -p video-translate-overlay --lib api::duration_tests:: -- --nocapture` | 2 passed. Existing resumed-video and duration behavior. |
| `cargo test -p video-translate-overlay --lib pipeline::worker_tests:: -- --nocapture` | 6 passed. Existing background/retry, completed-cue retention, cost stop, and credential refusal behavior. The retry case took 170.73 seconds. |
| `npx vitest run -c vitest.ui.config.ts tests/ui/updates.test.ts tests/ui/preferences.test.ts tests/ui/watch.test.ts tests/ui/library.test.ts` | 38 passed in Chromium. Includes both platform links, escaped notes, manual results, foreground coalescing, and a late response after dismissal. |
| `npm run build` | Passed. |

The duration/worker checks precede the Windows dialog and packaging repairs;
their exercised code and the affected fixture constructors did not change after
those passes. No whole repository gate or final verification was dispatched.

## Actual application observations

All fixtures used disposable data and a dummy key. No paid translation was
requested. Logs, screenshots, database snapshots, and machine-readable receipts
are retained locally in the ignored `artifacts/update-acceptance/` directory.

The real Rust local HTTP API and Chromium were exercised with
`node artifacts/update-acceptance/web.mjs`. The API selected 0.1.7 and its exact
Windows URL, persisted dismissal, and revealed that version on a manual check.
Malformed metadata returned 503 and a failure message; an empty valid release
list returned a separate no-update result. Layout checks passed at 384 by 854
and 1280 by 720, with no horizontal overflow. The HTTP build identified `74f393e`;
its update API and selection implementation are unchanged in the final source.

Android used the installed 0.1.6 debug APK as the baseline and upgraded it to
0.1.7 under disposable emulator user `VtoReleaseAcceptance` (ID 10). Actual app
actions stored a key in Android Keystore and a playlist containing a saved
video. Reopening after the APK update preserved the exact library response,
saved cues, job cost of 12.5 KRW, and readable masked key. Launch and manual
notices displayed the version, notes, and exact ARM64 URL. Dismissal suppressed
automatic checks and manual checking revealed the release again.

The native Android foreground path was exercised while a video site occupied
the app's single WebView: Home, then return to the app, showed the 0.1.8 fixture
prompt with both actions. Dismissal followed by another return did not repeat
that prompt. Native notice screenshots identify build `d84ebe8`. The final
packaging changes do not alter that Android lifecycle implementation.

The final production ARM64 APK from `f8ce388` was also installed in place and
launched. The API 34 x86-64 emulator advertises ARM64 support and runs this APK
through its native bridge. Android reported `primaryCpuAbi=arm64-v8a`, version
name 0.1.7, and version code 1007. The real settings screen read the existing
secure key, the library retained both saved videos and the playlist, and a
manual check used the actual public GitHub source rather than the debug fixture
left in private storage. Every stored database row and the sealed credential
file matched the pre-upgrade snapshot. There was no physical Galaxy S21 run or
claim about physical-device performance.

Windows used installed NSIS debug builds with an isolated `WIKI_NATIVE_PROFILE`
and folder-specific real Credential Manager entry. The same saved-data snapshot
and readable masked key survived process restart and installed-binary changes.
Actual WebView2 clicks exercised launch notice, dismissal, manual reveal, and
download selection. The notice was 672 CSS pixels wide, with 44-pixel controls,
at a 1024-pixel viewport. Android controls were also 44 CSS pixels high, with no
horizontal overflow at its 412-pixel viewport.

The Windows video-site prompt was exercised in an installed build using the
window UI thread repair committed as `fe61ecb`. Native Win32 inspection verified
the prompt belonged to the tested process and contained 0.1.8, the feature
summary, and the exact Windows installer URL. After dismissal, IPC remained
responsive and the automatic result was suppressed. The final `f8ce388` change
only gates development executables out of distribution; the dialog code and
effective production runtime features match this tested repair.

External navigation was observed beyond the app's IPC result: Android Chrome's
address bar and the Windows browser accessibility value both contained their
exact platform-specific 0.1.7 asset URLs. These fixture actions establish the
navigation behavior, not artifact availability. Windows production-user data
was not modified for acceptance; its native retention checks used the isolated
debug profile and the unchanged production credential/data identities.

## Public channel acceptance

Before publication the anonymous releases endpoint returned HTTP 200 and an
empty array. The user then selected `Publish 0.1.7 (Recommended)`, approving the
prepared files, notes, checksums, and tag at `f8ce388`. The annotated `v0.1.7` tag
was pushed to that exact commit and the normal GitHub Release was created with
the approved notes and artifacts. It is neither a draft nor a prerelease.

`node artifacts/update-acceptance/public-release.mjs` passed: a fresh HTTP client
used no authorization header, received HTTP 200 for release metadata and both
binary downloads, and matched each downloaded byte count and SHA-256 against
the approved local files above. GitHub redirected the asset responses to
`release-assets.githubusercontent.com`. The published body also matched the
approved notes. `git rev-parse 'v0.1.7^{}'` confirmed the approved source commit.
No credential is embedded in the app or required by this channel.

The actual Windows Tauri app and installed Android debug APK were rebuilt at
`6cd0d5d`, whose runtime source matches the published candidate. Both used the
existing isolated saved-data profiles with `update-installed.txt` set to 0.1.6.
There was no `update-fixture.json` on either platform: the only override was the
installed-version comparison. Each native app fetched the real public metadata,
selected 0.1.7, displayed its actual feature summary and correct platform URL,
persisted dismissal, revealed it through the settings button, and exercised the
download link. The secure key status, library, cues, and 12.5 KRW saved job cost
matched the pre-upgrade snapshots. No unrelated production release was created.

`node artifacts/update-acceptance/native-public.mjs windows` passed. The default
Windows browser downloaded the 4050199-byte installer into Downloads; its actual
file hash was `6776b1041198786f721e590af921a92bb7e289f9cdb4b1c0dfb52d521a99b591`.
Android Chrome received the exact public APK URL and displayed its standard APK
download confirmation. After confirming that approved file, the 24545528-byte
APK in the isolated user's Downloads matched
`c86c781d03ee6f19e38fe854ad37c1c54673bf0072b32d639f34cd11738bcc04`.
The Android handoff was verified in two phases while the app WebView paused
behind Chrome, then completed after return to the app with
`python artifacts/update-acceptance/android-public-download.py verify-only` and
`node artifacts/update-acceptance/native-public.mjs android resume`. Dismissal
remained effective and no download-opening error appeared.

The anonymously downloaded production ARM64 APK was then installed over the
isolated app. Its real settings screen reported no newer release for installed
0.1.7 against public 0.1.7, ignoring the debug-only 0.1.6 comparison file. The
secure key remained readable, and every saved database row and sealed credential
file still matched the pre-upgrade snapshot. Both
`python artifacts/update-acceptance/android-ui.py` and
`python artifacts/update-acceptance/android-production.py after` passed.

Public receipts and screenshots are retained under the ignored
`artifacts/update-acceptance/` folder: `public-release.json`, the two
`*-public.json` native receipts, `*-browser-download.json`, and the public launch
and manual-notice screenshots. The final verification limitations remain those
stated above: Android hardware was an emulator, and Windows retention used a
separate native debug profile rather than the production user's data.

The full repository gate and final verification are server-owned and remain
pending. They are not replaced by the focused checks above.

## Test cleanup audit

Retained all current checks in `updates_tests.rs`, the command/Android ACL tests,
the two affected Rust fixture test files, `contract-tests/tests/settings.rs`,
and the four affected UI test files. Parser, transport, native lifecycle, and
UI race checks guard distinct boundaries. No test was removed; no obsolete
skip or xfail was introduced or retained for a removed update path. Changed
tracked text files were decoded as UTF-8 and checked for the absence of a BOM.
