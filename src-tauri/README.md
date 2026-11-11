# Backend (`src-tauri`)

The Rust side of the app, as designed in `docs/design/backend.md` and `docs/design/api.yaml`.

| File | What it does |
| --- | --- |
| `src/commands.rs` | Tauri commands P1-P8 and the ACL. `capabilities/main.json` gives the app's own screen every command. `capabilities/overlay.json` gives site pages only P6-P8. |
| `src/api.rs` | What each command does: the YouTube sign-in gate, key masking, job creation rules (reuse, 409, 422, 428), progress and subtitles. |
| `src/pipeline.rs` | The background worker. It runs one job at a time: download, then write on the device, then translate, then notify. It records every step in SQLite, so a killed process resumes from the last written region. |
| `src/audio.rs` | Decoding to 16 kHz mono (WAV here, everything else through Media Foundation on Windows) and the voice-activity regions the model is given. Cue times are always on the source timeline. |
| `src/media.rs` | Media URL policy (http/https only, no loopback or private addresses), HLS/DASH/DRM/live probing, and the download itself. |
| `src/gemini.rs` | `gemini-3.1-flash-lite` calls. One glossary order runs first, then numbered batches of 40 segments with no timestamps. This file also holds pricing, cost and the 1,000 KRW per hour limit. |
| `src/store.rs`, `src/db.rs` | The SQLite schema and a small binding to the OS SQLite (`winsqlite3.dll` on Windows). |
| `src/http.rs` | Outbound HTTP through WinHTTP on Windows. |
| `src/youtube.rs` | YouTube URL parsing and the best-effort audio extractor. |
| `src/platform.rs` | Windows Credential Manager for the key, and the slot for the on-device model. |
| `src/bin/vto-contract-host.rs` | The contract host that `contract-tests` drives. It is the real core on Tauri's mock runtime, with the stand-ins that `contract-tests/src/lib.rs` lists. |

Run the tests with the repository gate:
`npm ci --prefer-offline --no-audit --no-fund && node scripts/contract-tests.mjs artifacts/junit.xml`.

## Not in this build yet

- whisper.cpp is not linked yet. `platform::UnbundledModel` makes a job that reaches the writing stage fail with `write_failed`; the contract tests use the fake whisper.cpp, as the design specifies.
- Android still lacks the Keystore, MediaCodec decoding, the foreground service and HTTPS. Off Windows, only WAV decodes and only plain-HTTP requests work.
- The app shell (`tauri.conf.json`, the Svelte screens, the real webview and notification bindings) does not exist yet. The `Browser` and `Notifier` traits in `src/lib.rs` are where those bindings go.
- The YouTube extractor is unverified against the live service. When it fails, the job is refused with `youtube_unavailable` (R20).
