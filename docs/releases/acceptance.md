# Release update notice acceptance

The 0.1.7 candidate is prepared locally. Publication and public artifact
verification require the user's approval and remain pending. This record does
not declare a fixture-only channel complete or claim that the repository gate
passed.

## Candidate for approval

The packaging script completed both platforms from clean source commit
`f8ce38868089674d17c8a8410aaecb05d979e8fb`. Versions match at 0.1.7; Android's
version code is 1007. Outputs are in the ignored `artifacts/releases/0.1.7/`.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `video-translate-overlay-0.1.7-arm64.apk` | 24545528 | `c86c781d03ee6f19e38fe854ad37c1c54673bf0072b32d639f34cd11738bcc04` |
| `video-translate-overlay-0.1.7-windows-x64-setup.exe` | 4050199 | `6776b1041198786f721e590af921a92bb7e289f9cdb4b1c0dfb52d521a99b591` |

The approved-body candidate is `docs/releases/0.1.7.md`, SHA-256
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

## Remaining publication acceptance

Unauthenticated requests to the public GitHub releases endpoint returned HTTP
200 and an empty array. Installed Windows and the production Android app also
checked this actual source successfully. No GitHub credential is embedded in
the application. Tag `v0.1.7` did not exist remotely at preparation time.

The user must approve the two files, hashes, notes, and first outward
publication. After authorization, push the tag for the candidate source commit,
publish the normal approved release, download both assets without credentials,
compare their hashes, and repeat native selection/navigation against that actual
metadata using the isolated older-version comparison. No unrelated later
production release is needed. Until those checks pass, public download
reachability and live-channel acceptance remain pending.

The full repository gate and final verification are server-owned and remain
pending. They are not replaced by the focused checks above.

## Test cleanup audit

Retained all current checks in `updates_tests.rs`, the command/Android ACL tests,
the two affected Rust fixture test files, `contract-tests/tests/settings.rs`,
and the four affected UI test files. Parser, transport, native lifecycle, and
UI race checks guard distinct boundaries. No test was removed; no obsolete
skip or xfail was introduced or retained for a removed update path. Changed
tracked text files were decoded as UTF-8 and checked for the absence of a BOM.
