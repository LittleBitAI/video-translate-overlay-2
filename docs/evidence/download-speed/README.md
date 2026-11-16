# Audio download speed: execution evidence

These are the raw observations behind
[the measurement record](../../design/download-speed.md). Each file is
copied from the phone's shell output or the app's log, without edits. The only
changes: `#` header lines, dropped `adb push` progress lines, and line endings
made LF. The app logs keep only the app's own lines and the system's lines
about the app, with any URL replaced by `<url>`. Signed URLs,
network names and addresses were never printed into these files.

## Build and source identities

| What | SHA-256 | Source |
| --- | --- | --- |
| Harness, current head | `1e82fe4dae8e1ec230ab94e2ff84159caf81a9c9d99af976211b3d9aba83cda1` | `b152467`, same command as the next row |
| Windows app, current head | `2f26bdb959a52986a001532eef020f4f6dc8922f84034ecce7c65fb3b28a8d30` | `b152467`, `cargo build -p video-translate-overlay --features app --bin video-translate-overlay` (debug); the running app reported `window.__WIKI_NATIVE_BUILD_HEAD__` = `b1524675c22d481f29a545a4f6a8c9c7ee889b95` |
| Harness, in-app head | `3becb121ed69e1a6fcdc18508960e56e5889163934152b4441f8eaf5950ae89d` | `c632ca1`, `cargo build -p video-translate-overlay --release --target aarch64-linux-android --example download_timing --no-default-features` |
| Harness, baseline (reproducible) | `6b133ed43e9167ff1f658485954d90c52d36c70a2b94dfa602be7ede874cefe2` | `git archive 678515b`, plus `src-tauri/examples/download_timing.rs` from `c632ca1` (SHA-256 `1acf13445d988c176b9b4e00f4f473f22cd67e9c9fd51bc5f20421a88d9ef8db`); same command |
| Harness, baseline (session 1) | `93002709039984cb20469a9b8178523dda934cccfe95b4119636fea3c665fb8a` | `678515b` with [`baseline-timing-lines.patch`](baseline-timing-lines.patch) and that day's harness file (not kept in Git) |
| Harness, session 1 final | `3eff97df39d584b0e59313a0fe6df1698516a65794df2bb1b560e4d856f4ecef` | Uncommitted tree whose tracked changes from `678515b` equal those of `ec29dc8` line for line (SHA-256 of the changed lines, both: `327cab1e293bce6b7a199118537076e515372fe358bc67dc609683415c5be434`). The files that tree lacked are tests and the harness, which are not part of the library. |
| Harness, session 1 candidate | `38e9604196a668d2fcc2bb212d98e9a0cf5a26bedc8ea4d471d4b249c6624019` | Transfer code before `ec29dc8`; not in Git |
| App APK, in-app head | `c4783432ddcbc4cd9a6012960b303d81bc03b5acdd7b58c690fabd278cebb03c` | `c632ca1`; `libvto.so` inside it `b288e50ca7ebed9fab52f47a274a04a8aff989c226b05ec5393130621af521b2`, built by `cargo tauri android build --apk --target aarch64 --features app` from a clean `src-tauri` and `src` at `c632ca1`, packaged as a debuggable arm64 debug wrapper and signed with the existing development key (certificate SHA-256 `fa6da2233c2062b48baf20b3562eacacce774da80f2e4bb3d20ec3a6a527ec28`). The installed `base.apk` on the phone hashed to the same value. |

The speech model fetched in every run hashed to
`422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898`, the value
pinned in `src-tauri/src/whisper.rs`.

## Which source each receipt binds

The current head is `b152467`. Its harness runs, the Windows app run and the
offline file were made from it. The Android in-app run used the `c632ca1` APK
and is reused. From `c632ca1` to `b152467` (`git diff --stat c632ca1 b152467`)
the app's code changes in three files only:

- `src-tauri/src/http.rs`: the response keeps its `Date` header;
- `src-tauri/src/media.rs`: `identifies_revision` joins pieces under a
  `Last-Modified` only when `Date` is at least one second later;
- `src-tauri/src/media_ranged_tests.rs`: the tests of that rule.

The rest are documents and these receipts. Model preparation, P7, restart and
the ready notice do not read either header. YouTube's audio answers with a
`Date` long after its `Last-Modified`, so on that URL both heads take the same
ranged path. The `b152467` harness and Windows runs re-measured that path and
wrote the same bytes.

## Environment

- Phone: `getprop ro.product.model` = `SM-G996N` (Galaxy S21+), Android 15,
  app `dev.vto.overlay` labelled 0.1.7. Connected to the PC by USB debugging.
- Network, session 2 (`dumpsys wifi`): 5180 MHz, 802.11ac, RSSI −28 dBm,
  585 Mbps link.
- Video: `zjkBMFhNj_g` for the harness runs (audio itag 140, 58,064,373 bytes).
  The in-app run used `QDX-1M5Nj7s`, "MIT Introduction to Deep Learning (2023)
  | 6.S191", player duration 3,491.5 s, because the first video is already
  translated in the library.
- The signed audio URL came from the app's own YouTube WebView session, through
  the overlay's player request. On that URL each ranged answer carried
  `Last-Modified: Fri, 13 Sep 2024 08:50:31 GMT` and
  `Content-Range: bytes a-b/58064373`, with no `ETag`. A URL issued the same
  way on 2026-11-16 answered its first two 10 MiB ranges with that
  `Last-Modified` and `Date: Mon, 16 Nov 2026 00:41:20 GMT`. So the ranged
  path applies under the dated revision rule of the current head too.

## Files

| File | What it holds |
| --- | --- |
| [`phone-b152467-runs.txt`](phone-b152467-runs.txt) | Five harness runs at the current head, 2026-11-16 00:50–00:52 UTC, same phone, network and URL source as session 2, `--model --reuse` |
| [`windows-b152467.log`](windows-b152467.log) | The Windows app's own log (stderr) at the current head, from launch to the job resumed after a forced stop |
| [`windows-b152467-overlay.json`](windows-b152467-overlay.json) | The Windows overlay's status text after the press, read every 100 ms over the site WebView's DevTools; P7 before and after the restart; the audio and model files' sizes and digests |
| [`offline-b152467.txt`](offline-b152467.txt) | Focused offline checks at `b152467`, and the revision table failing under the `Last-Modified`-alone rule |
| [`phone-c632ca1-runs.txt`](phone-c632ca1-runs.txt) | Five harness runs at the in-app head, session 2, `--model --reuse`: per step milliseconds, bytes and digests |
| [`phone-baseline-runs.txt`](phone-baseline-runs.txt) | Four baseline runs: two in session 1, two in session 2 (run 4 with the reproducible baseline harness) |
| [`phone-ec29dc8-runs.txt`](phone-ec29dc8-runs.txt) | Five session 1 runs of the first implementation |
| [`phone-candidate-runs.txt`](phone-candidate-runs.txt) | The two session 1 candidate runs, 62.2 s and 6.2 s |
| [`phone-c632ca1-pieces.txt`](phone-c632ca1-pieces.txt) | One against four parallel 10 MiB ranges |
| [`app-c632ca1.log`](app-c632ca1.log) | The app's own log lines (`vto`) and the system's process, force-stop and notification lines for the app, from launch to the ready notice |
| [`app-c632ca1-overlay.json`](app-c632ca1-overlay.json) | The overlay's status text after the press, read every 100 ms from its shadow DOM over the WebView's DevTools |
| [`app-c632ca1-p7-writing-resume.json`](app-c632ca1-p7-writing-resume.json) | P7 (`get_job`) called through the app's own Tauri bridge every 100 ms after the writing-resume restart |
| [`app-c632ca1-ready.json`](app-c632ca1-ready.json) | P7 and P8 (`get_job`, `get_subtitles`) through the app's bridge after the ready notice: the job's state and cost, the cue count and range, and the cues at 120, 1,800 and 3,400 s |
| [`app-ec29dc8.log`](app-ec29dc8.log), [`app-ec29dc8-overlay.json`](app-ec29dc8-overlay.json) | The same two records for the session 1 in-app run of `zjkBMFhNj_g`: the overlay showed `오디오 내려받는 중 0%` at 0.8 s, then `음성 인식 모델 준비 중(처음 한 번) 0%` at 1.6 s, and the job was ready 3,582 s after the press |
| [`offline-c632ca1.txt`](offline-c632ca1.txt) | Focused offline checks at `c632ca1`, and the two new tests failing on the code before the fix |

## What the Windows app run shows (`b152467`)

Windows 11 Home 10.0.26200 on the PC, on the same home network as the phone.
The app ran with its own data folder (`VTO_APP_DATA_DIR`) and its own WebView2
profile, both under the ignored `artifacts/` folder, so it shared nothing with
an installed copy. A dummy Gemini key was stored through the app's bridge, and
a loopback stand-in answered it; Google's Gemini was never contacted. The
video page opened in the app's own site window, signed out. The detect button
was pressed in the overlay at 2026-11-16T00:58:13.280Z. The speech model
(574,041,195 bytes) had not been installed, so this was a first use.

| Time after the press | Observation | Source |
| ---: | --- | --- |
| 0.1 s | Overlay `작업을 만드는 중…` | overlay file |
| 0.2 s | Overlay `오디오 내려받는 중 0%` | overlay file |
| 0.5 s | Overlay `음성 인식 모델 준비 중(처음 한 번) 0%`; worker took the handed source (`reused=true`, 0 ms) | overlay file, log |
| 51.5 s | Overlay back to `오디오 내려받는 중 50%`; model fetched (51,153 ms) | overlay file, log |
| 58.5 s | Overlay `말을 글로 옮기는 중 0%`; audio transferred just before: 58,064,373 bytes in 6,718 ms, first try | overlay file, log |
| about 100 s | Decoded: 57,403,814 samples (41,399 ms, unoptimized debug build) | log |
| 140.9 s | P7: `stage: writing`, `preparing_model: false` | overlay file |
| 157.6 s | App force-stopped (`taskkill /F`), relaunched 4.9 s later | log header |
| 185.7 s | P7 after reopening the page: `stage: writing`, `preparing_model: false`, no failure; log `resuming job … at writing` | overlay file, log |

The saved audio hashed to
`a1fadc741fa289c8664bae7c221ffc3ab4b70c6c0a17be41bd9cf9810fd6583c`, the same
file as every phone run. Its sample count equals the phone's decode. The model
hashed to `394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2`.
The run was stopped while writing; the Windows debug build was not run to the
ready notice. The log lines carry no clock; rows timed from them alone are
the overlay's time plus the logged duration.

## What the Android in-app observations show (`c632ca1`)

Times are on the phone's clock, counted from the press (`pressed_at_ms` in
the overlay file).

| Time | Observation | Source |
| ---: | --- | --- |
| 0.1 s | Overlay `작업을 만드는 중…` | overlay file |
| 1.7 s | Job created; P6 probed the source once (1,349 ms) | log |
| 1.8 s | Overlay `오디오 내려받는 중 0%`: the first P7 poll, before the worker took the job | overlay file |
| 1.9 s | Worker took the handed source (`reused=true`, 0 ms) and began preparing the model | log |
| 2.2 s | Process force-stopped from adb during the model fetch | log |
| 5.8 s | App relaunched; job resumed at downloading; fresh probe (`reused=false`, 335 ms) | log |
| 14.2 s | Model fetched and verified (7,631 ms) | log |
| 35.0 s | Audio transferred: 56,507,767 bytes in 20,774 ms | log |
| 41.7 s | Decoded (6,707 ms); writing starts | log |
| 94.7 s | Process force-stopped again; the model was moved aside | log |
| 95.6 s | App relaunched; job resumed at writing | log |
| 97.6 s | P7 `stage: writing, preparing_model: true` from the first call | P7 file |
| 102.6 s | P7 `preparing_model: false`; model fetched again with the pinned hash | P7 file |
| 112 s | App sent to the home screen; the job's foreground notice stayed (`자막 만드는 중 26%` at 32 minutes) | `dumpsys` |
| 6,598.5 s | Ready notice posted on channel `vto-ready`; log line `job … ready`; the app was on the home screen | log |
| after | P7: `stage: ready`, `percent: 100`, `preparing_model: false`, `cost_krw: 276.29`; subtitles: 755 cues, 1,180–3,490,540 ms, sample cues at 2, 30 and 57 minutes | ready file |

Not observed in this build: the overlay text `음성 인식 모델 준비 중(처음 한 번)`
on screen. The first force-stop came 0.3 s into the model fetch, before the
overlay's next one-second poll. The fetch after relaunch ran while the app
showed its library, not the video page. The flag the text is drawn from was
observed on the phone through P7 above. The text itself was seen on screen in
session 1 at the `ec29dc8` build, 1.6 s after the press. The overlay and
first-screen tests in the offline file check the mapping from the flag to the
text.
