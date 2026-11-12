# Backend (`src-tauri`)

The Rust side of the app, as designed in `docs/design/backend.md` and `docs/design/api.yaml`.

| File | What it does |
| --- | --- |
| `src/commands.rs` | Tauri commands P1-P8 and the ACL. `capabilities/main.json` gives the app's own screen every command. `capabilities/overlay.json` gives site pages only P6-P8. |
| `src/api.rs` | What each command does: the YouTube sign-in gate, key masking, job creation rules (reuse, 409, 422, 428), progress and subtitles. |
| `src/pipeline.rs` | The background worker. It runs one job at a time: download, then write on the device, then translate, then notify. It records every step in SQLite, so a killed process resumes from the last written region. |
| `src/audio.rs` | Decoding to 16 kHz mono (WAV here; everything else through `src/decode.rs` on every OS, with Media Foundation only as a fallback on Windows for what it cannot open; at most six hours of output, and sample rates of 4-384 kHz, whatever a file's header or timestamps claim) and the voice-activity regions the model is given. Cue times are always on the source timeline. |
| `src/decode.rs` | Decoding on every OS: symphonia for MP4/M4A, ADTS AAC, MP3 and WebM (Opus through libopus, built from source by cmake for the target; Vorbis), plus a small MPEG-TS demuxer for HLS `.ts` segments. Audio is placed by its timestamps on the timeline players use (TS: from the earliest PTS of any stream; MP4: the audio track's edit list played through, delays, cuts, skips and repeats alike, at rate 1; HLS with `EXT-X-DISCONTINUITY` or a changed `EXT-X-MAP`: each section decoded on its own after its own init section, from its first timestamp, and placed where the playlist's EXTINFs put it; a section that does not decode fails the file rather than leave a stretch unwritten), so audio missing while the video runs on stays silence and later speech keeps its time. Its tests run on Windows against Media Foundation. |
| `src/media.rs` | Media URL policy (http/https only; no loopback or private addresses, checked after DNS resolution and again on every redirect hop, variant, init section and segment; a name that does not resolve is refused; every request, http or https, connects to the address that was checked, with the name kept for `Host`, SNI and the certificate, and the connected address is checked again before a response is used), HLS/DASH/DRM/live probing over the whole playlist (relative URIs resolve against where redirects land; the first piece, the first segment and the last must answer a one-byte request, or the playlist is refused as `segment_unavailable`; the first section's init section must list a sound track and a TS segment's PMT an audio stream, or it is refused as `not_audio`; a master's variants whose CODECS name no audio are passed over), protected MP4 files (`sinf`/`tenc`/`pssh` in `moov`, fetched by range when it comes after `mdat`; from a server without Range, read off one GET with up to 64 MiB skipped, past which the file is refused as `unverifiable`), `EXT-X-BYTERANGE`, `EXT-X-DISCONTINUITY` and init sections that change (each section's byte range, playlist time and init section saved beside the download as `<audio>.sections`; every HLS download has at least one section, so its timeline starts at the first segment as players start it), files taken only when their first bytes are a container the decoder reads (WAV, MP4/M4A, WebM, MP3, ADTS AAC, MPEG-TS; anything else is refused as `not_audio`) and, for MP4, MPEG-TS and WebM, when it lists a sound track (video-only files are refused as `not_audio` too), YouTube live and upcoming broadcasts refused even when they list audio, and the download itself. |
| `src/gemini.rs` | `gemini-3.1-flash-lite` calls. One glossary order runs first, then numbered batches of 40 segments with no timestamps. This file also holds pricing, cost and the 1,000 KRW per hour limit. |
| `src/store.rs`, `src/db.rs` | The SQLite schema and a small binding to the OS SQLite (`winsqlite3.dll` on Windows). |
| `src/http.rs` | Outbound HTTP through WinHTTP on Windows, and elsewhere through its own HTTP/1.1 client with rustls (ring, Mozilla's roots). Requests pinned to an address (media) use that client on Windows too, since WinHTTP cannot connect to a chosen address and still check the certificate against the name. Chunked bodies are streamed. Each response reports the address it really connected to. |
| `src/youtube.rs` | YouTube URL parsing and the best-effort audio extractor. |
| `src/platform.rs` | Windows Credential Manager for the key. |
| `src/android.rs` | Android: the key sealed with an Android Keystore AES-GCM key, the job's foreground service (with a partial wake lock, so the job goes on with the screen off) and its ongoing progress notification, and the ready notice whose tap opens the video. The Kotlin half is `gen/android/app/src/main/java/dev/vto/overlay/` (`VtoPlugin.kt`, `JobService.kt`). No page can call it: the ACL grants no plugin command. |
| `src/whisper.rs` | The on-device model (`app` feature): whisper.cpp with large-v3-turbo q5_0, fetched on the first job into the data folder and checked against a pinned SHA-256. It uses beam search, and the language is detected once per job. |
| `src/app.rs` | The app (`app` feature), a library module because Android loads it as `libvto.so`; `src/main.rs` runs it on the desktop. It has the settings screen (`../ui/index.html`), the site webview with the overlay bundle injected into every page and every frame in it (`src/overlay/page.ts`: one overlay per page, over its largest video, an iframe's included, and inside the player in full screen; `src/overlay/signin.ts`: a skip button on Google's sign-in pages, taken by itself when Google refuses an in-app sign-in) (its own "site" window on the desktop, the one window on Android, where `capabilities/overlay-android.json` applies), OS notifications, and a main window that only hides while a job runs. The ready toast on Windows launches `vto-open:<job id>`, a per-user protocol registered at start, so a click opens the video at any later time, after a restart too. A running app gets the launch through single-instance. |
| `build.rs` | For the `app` feature only: bundles `src/overlay/inject.ts` with `vite.overlay.config.ts` and runs tauri-build. |
| `android/whisper.toolchain.cmake` | The NDK toolchain for whisper.cpp in cross builds (`.cargo/config.toml` points there). |
| `src/bin/vto-contract-host.rs` | The contract host that `contract-tests` drives. It is the real core on Tauri's mock runtime, with the stand-ins that `contract-tests/src/lib.rs` lists. |

Run the tests with the repository gate:
`npm ci --prefer-offline --no-audit --no-fund && node scripts/contract-tests.mjs artifacts/junit.xml`.

Build and run the app on Windows. This needs `npm ci` first and libclang for whisper-rs's bindgen, for example from `pip install libclang`:
`LIBCLANG_PATH=<dir with libclang.dll> cargo run --release -p video-translate-overlay --features app --bin video-translate-overlay`.
`VTO_WHISPER_MODEL=<file>` uses an existing model file instead of fetching one.
The real model has its own opt-in test:
`VTO_WHISPER_MODEL=<file> cargo test --release -p video-translate-overlay --features app --test whisper_model -- --ignored`.

## Build for Android

The project is in `gen/android` (Tauri's Android Studio project, with the Kotlin plugin and service). It needs JDK 17, the Android SDK (platform 36, build-tools 35), NDK 27, Ninja on `PATH`, `LIBCLANG_PATH` as above, `rustup target add aarch64-linux-android` (or `x86_64-linux-android` for the emulator), and the Tauri CLI 2 (`cargo install tauri-cli --version ^2`):
`cargo tauri android build --apk --target aarch64 --features app` (`--debug --target x86_64` for the emulator).
On Windows the CLI ends by symlinking `libvto.so` into `gen/android/app/src/main/jniLibs/<abi>/`, which needs developer mode. Without it, copy `target/<triple>/debug/libvto.so` and the NDK's `libc++_shared.so` there, then run `gradlew assemble<Abi>Debug -x rustBuild<Abi>Debug` in `gen/android`.
A cmake cache from another generator makes whisper.cpp fail; delete `target/<triple>/*/build/whisper-rs-sys-*` then.

## Not in this build yet

- Android is built and tested on the emulator only (Android 14, x86_64): the key in the Keystore, a job running on with the app in the background under the foreground service, on-device writing, the progress and ready notices, and the tap that opens the video. A real Galaxy S21 Ultra (aarch64) has not run it, and no release signing is set up.
- Linux and macOS have no key store yet: the key is refused rather than written in plain text.
- On the desktop, progress shows in the window title.
- Media downloads connect directly, so a system proxy that Windows would apply is not used for them (Gemini calls still go through WinHTTP).
- The YouTube extractor is unverified against the live service. When it fails, the job is refused with `youtube_unavailable` (R20).
