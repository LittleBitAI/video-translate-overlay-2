# Approved Android and Windows releases

Distribution uses public GitHub Releases in `LittleBitAI/video-translate-overlay-2`,
as chosen in `docs/design/backend.md`. The source repository is public. The app
requests the [public releases endpoint](https://docs.github.com/en/rest/releases/releases#list-releases)
without authentication. No GitHub token is stored in the app or an APK.

## Release contract

Use a stable tag `vMAJOR.MINOR.PATCH`, with three nonnegative integer components,
no leading zeroes, prerelease suffix, or build suffix. Package, Cargo, Tauri,
Android version name, and Windows installer version must match. Android's Tauri
version code must increase. A release becomes approved only after the user
approves the concrete artifacts and notes and authorizes its publication.

Approved release bodies contain the exact line `<!-- vto-approved:v1 -->` and
a `## Features` section. Write one to three brief plain-text feature bullets,
at most 600 characters in total, ending at the next `## ` heading. Do not put
download links, HTML, credentials, or operational build logs in feature notes.
The app displays the summary as untrusted text without interpreting Markdown or
HTML. The prepared body for this task is `docs/releases/0.1.7.md`.

An announced release must be published, not a draft or prerelease. Its applicable
asset must have GitHub state `uploaded`, a positive size, and exactly this name
and browser download URL:

| Platform | Asset | HTTPS destination |
| --- | --- | --- |
| Android ARM64 | `video-translate-overlay-VERSION-arm64.apk` | `https://github.com/LittleBitAI/video-translate-overlay-2/releases/download/vVERSION/video-translate-overlay-VERSION-arm64.apk` |
| Windows x64 | `video-translate-overlay-VERSION-windows-x64-setup.exe` | `https://github.com/LittleBitAI/video-translate-overlay-2/releases/download/vVERSION/video-translate-overlay-VERSION-windows-x64-setup.exe` |

The app selects the highest numeric version greater than the installed version
that passes all checks. A missing Android artifact does not prevent an eligible
Windows notice, and vice versa. Unsupported platforms do not receive a download.
Unapproved development builds belong in CI artifacts, not this distribution
channel. Checks inspect the 100 most recently created GitHub Releases and at most
512 KiB of JSON; keep approved current releases inside that window. Oversized or
malformed responses fail the check rather than claim the app is up to date.

## Build and prepare locally

1. Commit the candidate source, matching versions, and feature notes. Install
   dependencies with the README instructions. Use the existing JDK 17, Android
   SDK 36/build-tools 36.0.0, NDK 27.2.12479018, Ninja, libclang, and Tauri CLI 2.
   Set `JAVA_HOME`, `ANDROID_HOME`, `NDK_HOME`, and `LIBCLANG_PATH`; add their tools
   and the installed Tauri CLI to `PATH`. Build on Windows with the C++ toolchain.
2. Back up the existing Android signing keystore securely outside Git. Do not
   generate a new key. `npm run package:release` builds the ARM64 release APK
   and the Windows x64 NSIS installer. To repeat one platform, append
   `-- -Platform android` or `-- -Platform windows`. The script handles Windows
   JNI symlink restrictions by copying the freshly compiled libraries and
   running Gradle without rebuilding Rust.
3. Review `artifacts/releases/VERSION/`: the two installable files,
   `release-notes.md`, `SHA256SUMS.txt`, Android certificate/package reports, and
   `manifest.json` identifying the source revision and notes hash. `dirty` must
   be false for an approved candidate. These local outputs are ignored by Git.
4. Verify Android signing continuity, installability, versions, and data
   retention in isolated native profiles. Exercise both download actions.
   Ask the user to approve these exact artifacts, hashes, notes, and outward
   publication. Do not create a release or push a tag before approval.

Android retains package ID `dev.vto.overlay` and the 0.1.6 development certificate
SHA-256 `fa6da2233c2062b48baf20b3562eacacce774da80f2e4bb3d20ec3a6a527ec28`.
The script refuses another certificate. Its local default is
`%USERPROFILE%/.android/debug.keystore`, alias `androiddebugkey`, standard debug
password. Private overrides use `VTO_ANDROID_KEYSTORE`, `VTO_ANDROID_KEY_ALIAS`,
`VTO_ANDROID_STORE_PASSWORD`, and `VTO_ANDROID_KEY_PASSWORD`. Do not commit these
values or the keystore. Retaining this development key preserves installed data
but provides weaker signing protection than a private production key. Replacing
it is a separate migration decision; uninstalling to change it destroys local
credentials and data. See [Tauri's Android signing procedure](https://tauri.app/distribute/sign/android/).

Windows uses the existing identifier and a per-user NSIS installation. The
installer is currently unsigned and may trigger Windows publisher warnings.
No signing certificate is purchased or silently introduced. See
[Tauri's installer documentation](https://tauri.app/distribute/windows-installer/).

## Publish only after approval

1. Obtain approval for the source commit, files, notes, and hashes. Push the
   authorized tag pointing at that source commit. Publish `vVERSION` as a normal
   GitHub Release with the approved body and both exact assets; include
   `SHA256SUMS.txt`. Do not mark it as prerelease. A draft may hold the approved
   files during this step, but does not establish the public channel.
2. Use a fresh unauthenticated HTTP client to GET the release metadata and
   download both artifacts, following GitHub's asset redirects. Verify the
   downloaded SHA-256 values against the approved local hashes. Publishing is
   incomplete until metadata and files are anonymously reachable.
3. In isolated older-version native profiles, check the actual public release,
   invoke each platform's download action, and verify the external destination.
   Record the metadata, artifact hashes, native actions, and limitations in the
   acceptance record. Do not manufacture an unrelated later production release
   just to make an update notice appear.

## Checking, dismissal, and recovery

Launch and native foreground events check on their own worker. Local app screens
show an accessible dismissible banner. If a video site occupies the foreground,
a native in-app prompt shows the version and feature summary, with download and
dismiss actions; remote sites receive no new IPC permissions. There is no
background schedule, OS update notification, automatic download, or installer.
Native prompts run on the window's UI thread; only the update-check worker waits
for the user's choice.

Only one network request runs at a time per core. Automatic checks reuse results
for 60 seconds. HTTP operations time out after 5 seconds; callers wait at most
15 seconds. A slow underlying request remains the sole in-flight request until
it finishes, even after callers time out. Streams are bounded to 512 KiB plus
one byte and API redirects are refused. The update worker does not lock the
video worker, SQLite, or secure credentials.

`설정 → 업데이트 확인` bypasses the automatic cooldown, joins any in-flight check,
and can reveal a dismissed version. Manual failures have a retryable failure
message, separate from no eligible newer release. Automatic failures are silent.
`이 버전 알림 닫기` atomically saves the highest dismissed version in
`dismissed-update.txt` beside the existing database. That version and older
versions stay dismissed, including after a release is withdrawn. A newer version
may notify again. Video-site prompt dismissal and
successful download selection also record this version. Download navigation is
restricted to the exact expected HTTPS GitHub asset URL and uses the external
browser; GitHub then serves its own asset redirect. A failed browser launch can
be retried from settings.

## First manual upgrade and data retention

Installations through 0.1.6 have no update checker. Give those users the approved
APK or Windows installer link once, and have them install manually over the
existing app. Do not uninstall, clear Android app storage, remove the Windows
data directory, or change Windows user. Android's package/signing identity and
Windows' app/credential identities stay unchanged. The checker adds a small
dismissal file; it does not migrate or rewrite video, subtitle, playlist, or
credential storage. Back up important saved work before testing an upgrade.

Debug-only isolated acceptance may use `update-fixture.json` for deterministic
release metadata and `update-installed.txt` for an older installed version. The
latter also works with the actual public endpoint. These app-private files are
ignored by release builds; no alternate update server is accepted. Windows
debug profiles use `WIKI_NATIVE_PROFILE` and a folder-specific Credential Manager
entry; Android acceptance uses a disposable emulator user. Test fixture metadata
does not establish or prove a working public distribution channel.
