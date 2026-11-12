# Backend (`src-tauri`)

The Rust side of the app, as designed in `docs/design/backend.md` and `docs/design/api.yaml`.

| File | What it does |
| --- | --- |
| `src/commands.rs` | Tauri commands P1-P8 and the ACL. `capabilities/main.json` gives the app's own screen every command. `capabilities/overlay.json` gives site pages only P6-P8. |
| `src/api.rs` | What each command does: the YouTube sign-in gate, key masking, job creation rules (reuse, 409, 422, 428), progress and subtitles. |
| `src/pipeline.rs` | The background worker. It runs one job at a time: download, then write on the device, then translate, then notify. It records every step in SQLite, so a killed process resumes from the last written region. |
| `src/audio.rs` | Decoding to 16 kHz mono (WAV here, everything else through Media Foundation on Windows) and the voice-activity regions the model is given. Cue times are always on the source timeline. |
| `src/media.rs` | Media URL policy (http/https only; no loopback or private addresses, checked after DNS resolution and again on every redirect hop, variant, init section and segment), HLS/DASH/DRM/live probing over the whole playlist, `EXT-X-BYTERANGE`, and the download itself. |
| `src/gemini.rs` | `gemini-3.1-flash-lite` calls. One glossary order runs first, then numbered batches of 40 segments with no timestamps. This file also holds pricing, cost and the 1,000 KRW per hour limit. |
| `src/store.rs`, `src/db.rs` | The SQLite schema and a small binding to the OS SQLite (`winsqlite3.dll` on Windows). |
| `src/http.rs` | Outbound HTTP through WinHTTP on Windows. |
| `src/youtube.rs` | YouTube URL parsing and the best-effort audio extractor. |
| `src/platform.rs` | Windows Credential Manager for the key. |
| `src/whisper.rs` | The on-device model (`app` feature): whisper.cpp with large-v3-turbo q5_0, fetched on the first job into the data folder and checked against a pinned SHA-256. It uses beam search, and the language is detected once per job. |
| `src/main.rs` | The app (`app` feature). It has the settings screen (`../ui/index.html`), the "site" webview with the overlay bundle injected into every page, OS notifications, and a main window that only hides while a job runs. |
| `build.rs` | For the `app` feature only: bundles `src/overlay/inject.ts` with `vite.overlay.config.ts` and runs tauri-build. |
| `src/bin/vto-contract-host.rs` | The contract host that `contract-tests` drives. It is the real core on Tauri's mock runtime, with the stand-ins that `contract-tests/src/lib.rs` lists. |

Run the tests with the repository gate:
`npm ci --prefer-offline --no-audit --no-fund && node scripts/contract-tests.mjs artifacts/junit.xml`.

Build and run the app on Windows. This needs `npm ci` first and libclang for whisper-rs's bindgen, for example from `pip install libclang`:
`LIBCLANG_PATH=<dir with libclang.dll> cargo run --release -p video-translate-overlay --features app --bin video-translate-overlay`.
`VTO_WHISPER_MODEL=<file>` uses an existing model file instead of fetching one.
The real model has its own opt-in test:
`VTO_WHISPER_MODEL=<file> cargo test --release -p video-translate-overlay --features app --test whisper_model -- --ignored`.

## Not in this build yet

- Android has not been built: this machine has no Android target, NDK or SDK. It still lacks the Keystore binding, MediaCodec decoding, the foreground service, ongoing notifications and HTTPS. Off Windows, only WAV decodes and only plain-HTTP requests work, and the key store refuses to keep a key rather than write it in plain text.
- On the desktop, progress shows in the window title, and the ready toast does not open the page when clicked. The plugin has no click handler there.
- The YouTube extractor is unverified against the live service. When it fails, the job is refused with `youtube_unavailable` (R20).
