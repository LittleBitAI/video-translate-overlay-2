# Registered library verification

`verification.json` registers API, browser, Windows and Android acceptance for
the saved library. Registration is a plan for execution, not a passing receipt.
The repository gate stays unchanged. The host requires local approval before
executing these flows.

## What runs

The API and Chromium flows start their own `vto-web --stand-ins` at
`http://127.0.0.1:8136`. A shared media fixture binds
`http://127.0.0.1:8137`; an occupied port fails instead of attaching to an
existing server. Android reaches the fixture through `10.0.2.2:8137`.
The fixture is a three-second generated black video, without lecture content.

Each run creates a separate database from the pre-library schema at
`dc6fec7fd4a6`. It seeds two completed translations with original-timeline
cues, glossaries and a recorded cost of 12.5 KRW each. No real key is needed.
The native debug app points translation at the fixture; any translation
request fails the scenario. Existing contract tests also count transcription
and translation calls explicitly.

The native host launches the approved Windows executable on its own Windows
desktop with fresh application and WebView2 profiles. On Android it launches
a new emulator with writable disk images inside the attempt directory,
installs the approved debug APK, and seeds that installation. It does not
attach to an existing emulator, physical phone or user's app process. The
approved AVD supplies a template, not the test's writable data.

The shared UI scenario creates and renames playlists, changes order and
membership, reopens saved subtitles, restarts the process, cancels deletion,
confirms deletion from multiple playlists, and manually opens an untranslated
entry. Offline checks separately cover active jobs, failed file cleanup,
stale replies and old notification links.

## Prepare and approve a candidate

Use the root README's prerequisites and Android build instructions. Install
the locked Node dependencies and Chromium once. Build after committing the
candidate, so the embedded backend and screen revisions name that commit:

```powershell
npm run build
cargo build -p video-translate-overlay --bin vto-web
cargo build -p video-translate-overlay --features app --bin video-translate-overlay
cargo tauri android build --debug --apk --target x86_64 --features app
```

The native outputs are `target/debug/video-translate-overlay.exe` and
`src-tauri/gen/android/app/build/outputs/apk/x86_64/debug/app-x86_64-debug.apk`.
The README's JNI-copy fallback applies when Windows refuses Tauri's symlink.
The disposable API 34 x86-64 emulator is installed-app acceptance; it does
not claim a physical Galaxy S21 run.

This requires wiki-agent's owned WebView driver. In project verification
settings, set the browser tool to `playwright-chromium`, allow the local
origins above, and supply actual environment revisions for `api`, `dataset`,
`settings`, `windows` and `android`. Setup and cleanup can stay blank because
the scenarios own their data and process lifecycle.

Approve the following native scope using absolute paths and measured hashes:

```json
{
  "version": 2,
  "host": "webview",
  "node": {"path": "C:/absolute/path/node.exe", "sha256": "64 lowercase hexadecimal characters"},
  "applications": {
    "vto-windows": {
      "platform": "windows",
      "binary": {"path": "C:/absolute/path/video-translate-overlay.exe", "sha256": "64 lowercase hexadecimal characters"},
      "profile": "disposable-library-windows"
    },
    "vto-android": {
      "platform": "android",
      "binary": {"path": "C:/absolute/path/app-x86_64-debug.apk", "sha256": "64 lowercase hexadecimal characters"},
      "profile": "disposable-library-android",
      "adb": {"path": "C:/absolute/path/adb.exe", "sha256": "64 lowercase hexadecimal characters"},
      "emulator": {"path": "C:/absolute/path/emulator.exe", "sha256": "64 lowercase hexadecimal characters"},
      "avd": "installed API 34 x86-64 AVD template name"
    }
  }
}
```

`Get-FileHash -Algorithm SHA256` measures an asset; lowercase its hash for
approval. A rebuild requires updated approval. Stale binaries fail the
running-build check even when filenames match. Machine-specific approval
and environment files stay ignored by Git.

The host records ownership observations and screenshots in its temporary
artifact directory, reaps the native process tree and checks artifact hashes
before reuse. Registration does not start a Review Loop, publish a pull
request or replace the server-owned final gate.
