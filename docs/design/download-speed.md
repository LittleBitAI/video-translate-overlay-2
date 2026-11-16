# Audio download speed

The downloading stage of a one-hour YouTube lecture took about 30 minutes on a
phone that receives the same audio at Wi-Fi speed. The audio transfer was
almost all of it: YouTube paces one whole-file request of an audio stream to
about twice its playback rate (here 32 KB/s for 128 kbps AAC), whatever the
link can carry. Each byte range of the same signed URL is answered at full
speed.

## What changed

- A direct file, YouTube's audio included, is fetched in 10 MiB ranges written
  in order (`src-tauri/src/media.rs`, `download_file`). Each 206 must start at
  the byte the previous one ended, stay within the asked range and its
  `Content-Length`, and carry the complete length, `ETag` and `Last-Modified`
  of the first piece. Pieces are joined only when the first one names a single
  revision of the file. That takes a strong `ETag`, or a `Last-Modified`
  that the same response's `Date` puts at least one second in the past
  (RFC 9110 8.8.2.2). HTTP dates count whole seconds, so two revisions made
  within one second can share a `Last-Modified`. Once a second has passed,
  any later change carries a later date. YouTube's audio sends
  `Last-Modified` and `Date` and no `ETag`. On every range of the measured
  URL: `Last-Modified: Fri, 13 Sep 2024 08:50:31 GMT`, with `Date` that day,
  e.g. `Mon, 16 Nov 2026 00:41:20 GMT`. With no validator, only a weak
  `ETag`, or a `Last-Modified` that is too recent, undated or not in HTTP's
  preferred date form, two equal-length revisions could look alike. Such a
  file is taken in one whole-file response instead.
  A piece that drops is asked for again from the byte it reached; three failures in a row fail the attempt, which the worker then
  retries from the start as before. A server that answers the first range with
  200 sends the whole file in that one response, read as before. One request
  is open at a time and memory stays at a 64 KiB buffer.
- The worker's first attempt takes the source that job creation (P6) probed
  moments before, instead of probing it again. The hand-off is in memory and
  keyed by job; a retry, or a job resumed after the process restarts, probes
  afresh with the same URL policy and DRM checks.
- P7 reports `preparing_model: true` while the speech model is fetched: from
  before its request connects, through connection waits and retries, until
  the fetch ends or fails. This holds in the downloading stage and for a job
  resumed at writing. The overlay shows `음성 인식 모델 준비 중(처음 한 번)` and the
  first screen `음성 인식 모델 준비 중` instead of the audio download.
- The YouTube fallback extractor now skips non-default (dubbed) audio tracks,
  as the overlay's browser-session path already did. Both still choose the
  highest-bitrate AAC stream (itag 140 here), so the audio given to the speech
  model is unchanged.
- Each downloading step logs a `timing job` line (on Android to logcat, tag
  `vto`).

Not changed: HLS segments are still fetched one after another, unranged; the
measured slow path was YouTube's direct file. Parallel ranges were not
adopted. On the phone, four parallel 10 MiB ranges took 3.85 s against 1.15 s
for one in session 1 (10.9 MB/s against 9.1 MB/s). In session 2 they took
3.91–3.95 s against 1.03–1.19 s (10.6–10.7 MB/s against 8.8–10.2 MB/s). One
connection already uses nearly all of the link.

## Conditions

| | |
| --- | --- |
| Phone | Samsung Galaxy S21+ `SM-G996N`, Android 15, connected by USB debugging. The selected test device was called an S21 Ultra; adb reports this model. |
| Network | Home Wi-Fi, 5 GHz 802.11ac; no IPv6 route. Session 1 (2026-11-15 evening KST): 650 Mbps link, RSSI −41 dBm. Session 2 (2026-11-16 06:35–08:20 KST): 585 Mbps link, RSSI −28 dBm. The PC on the same network shares the public address the signed URL is bound to. |
| Video | "[1hr Talk] Intro to Large Language Models", `zjkBMFhNj_g`, 59:48. Audio itag 140, AAC-LC 128 kbps, 58,064,373 bytes. |
| Source URL | Issued by the app's own YouTube WebView session through the overlay's player request (`VISIONOS` client), the URL the overlay sends as P6's hint. One URL served each session; every run in a session used it. Its ranges carry `Last-Modified` and no `ETag`. |
| Baseline code | `678515b`, the branch's base. The app installed beforehand was a 0.1.7-labelled debug build with an unoptimized 255 MB core; all measurements used optimized cores. |
| Harness | `src-tauri/examples/download_timing.rs`, cross-built for `aarch64-linux-android` and run under `adb shell`. It calls the app's own probe, download and decode functions with the same portable HTTP client and rustls as the app. Without `--reuse` it probes twice as the baseline worker did. Each binary's SHA-256 and source are in [the evidence](../evidence/download-speed/README.md). |

## Results

Times are on the phone. "Writing starts" is probes plus transfer plus decode,
the point at which on-device transcription begins; the model fetch is added on
first use only.

The reviewed head is `c632ca1`. Session 2 measured it next to two baseline
runs on the same URL and network. The session 1 columns are the first
measurement, kept for the spread across days.

| Step | Baseline, session 1 (2 runs) | Baseline, session 2 (2 runs) | Session 1, `ec29dc8` (5 runs) | Session 2, `c632ca1` (5 runs) |
| --- | --- | --- | --- | --- |
| Source probes | 7.47 + 14.40 s; 0.26 + 0.09 s | 0.085 + 0.058 s; 0.085 + 0.055 s | one probe, 0.07–0.37 s | one probe, 0.06–0.11 s |
| Model fetch, first use (59,707,625 bytes) | 6.55 s (run 1) | not run | 6.8–8.8 s, median 8.7 s | 6.1–7.4 s, median 7.0 s |
| Audio transfer (58,064,373 bytes) | 1,796.7 s; 1,798.9 s | 1,799.7 s; 1,799.9 s | 5.67–6.03 s, median 5.80 s | 5.62–6.00 s, median 5.70 s |
| Decode | 10.5 s; 10.6 s | 4.8 s; 5.1 s | 8.8–11.3 s | 5.0–6.9 s |
| Writing starts, model installed | 1,829 s; 1,810 s | 1,805 s; 1,805 s | 14.5–17.7 s, median 15.3 s | 10.8–13.0 s, median 12.4 s |
| Writing starts, first use | 1,836 s (run 1) | — | 21.5–25.4 s, median 24.1 s | 16.9–20.3 s, median 19.1 s |

Baseline run 2 and the five `ec29dc8` runs of session 1 ran while the app was
transcribing the same lecture on four threads, which is why their decode is
slower. In session 2 the app was idle. Before the `ec29dc8` runs, two runs of
the same transfer code took 62.2 s and 6.2 s. The 62.2 s run is the only
ranged transfer above 7 s in 12. Probe times varied by network: 7–14 s in the
first baseline run, under 0.4 s since.

Across both sessions the baseline transfer stayed within 1,796.7–1,799.9 s,
and the ranged transfer within 5.6–6.0 s in ten of eleven harness runs (6.2 s
and 62.2 s for the earliest two). The difference is about 300 times the
spread of either.

The agreed targets were a transfer median of at most 60 s over at least five
runs with no run over 3 minutes, and writing starting within 90 s with the
model installed and 2 minutes on first use. Both final sets meet all of them.

Every baseline and final run wrote the same file (SHA-256
`a1fadc741fa289c8664bae7c221ffc3ab4b70c6c0a17be41bd9cf9810fd6583c`) and decoded
it to the same 57,403,814 samples at 16 kHz (3,587,738 ms, first sound at
64 ms, PCM SHA-256
`f04512f563bf3129af412b23e35f6dfe5d1f735eaefcb8e1ab807c2836d88fad`). Every model
fetch had SHA-256
`422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898`, the pinned
one. The speech model receives exactly the audio it received before, on the
same timeline.

## In the app, session 2 (`c632ca1`)

The reviewed head's APK (SHA-256
`c4783432ddcbc4cd9a6012960b303d81bc03b5acdd7b58c690fabd278cebb03c`, core
`b288e50ca7ebed9fab52f47a274a04a8aff989c226b05ec5393130621af521b2`) was installed
over the existing app, keeping its data. The installed `base.apk` hashed the
same. The first video was already translated, so this run used another public
lecture of the same length, `QDX-1M5Nj7s` (58:11). The model was moved aside so
the run was a first use. Each row cites its receipt in
[the evidence](../evidence/download-speed/README.md).

| Event | Time after the press |
| --- | ---: |
| Job created, source probed once (1,349 ms) | 1.7 s |
| Worker takes the handed source (`reused=true`, 0 ms), starts the model fetch | 1.9 s |
| Process force-stopped from adb during the model fetch; relaunched | 2.2–5.8 s |
| Job resumed at downloading; fresh probe (335 ms) | 6.6 s |
| Model fetched and verified (7,631 ms) | 14.2 s |
| Audio transferred: 56,507,767 bytes in 20,774 ms | 35.0 s |
| Decoded (6,707 ms); writing starts | 41.7 s |
| Process force-stopped again with the model moved aside; resumed at writing | 94.7–95.6 s |
| P7 says `preparing_model: true` while writing refetches the model, then `false` | 97.6–102.6 s |
| App sent to the home screen; the job continues under its foreground notice | 112 s |
| Ready notice (`vto-ready`), app still on the home screen | 6,598.5 s |

Writing started at 41.7 s on first use, including a restart. The agreed bound
is 2 minutes. This transfer took 20.8 s, slower than the harness's 5.6–6.0 s on
the other lecture's URL; it is one sample, and still a third of the 60 s target.

The job ended `ready` at 276.3 KRW, with 755 Korean cues from 1.2 s to
3,490.5 s of the 3,491.5 s video. The cues at 2, 30 and 57 minutes fall where
their lines are spoken (2:01 opens with the course's welcome).

Writing and translation took 6,387 s here, against 3,552 s for the session 1
lecture. The downloading stage's changes end at the decoded audio, and the
cause was not measured. During the first 30 minutes, baseline run 4 and the
parallel-piece check shared the phone, and the phone stayed on USB power
throughout.

## In the app, session 1 (`ec29dc8` library source)

The session 1 build was installed over the existing app, keeping its data: APK
SHA-256 `363bc2bf9e97bb222e562d5df4f4b92595733ce7f8f2ed4651c9c6387f650257`, core
`d49b6d88f3c8c9b9f20502ad59d57da1cb07029047febeabb5e31956d1d4c3c5`. Its
library source equals `ec29dc8`, as stated in
[the evidence](../evidence/download-speed/README.md). The model was moved aside
so the run was a first use.

| Event | Time after the press |
| --- | ---: |
| Job created, source probed once (119 ms) | 0.4 s |
| Overlay shows `음성 인식 모델 준비 중(처음 한 번)` (seen on screen; the screenshot shows the account, so it is not kept) | 1.6 s |
| Model fetched and verified (9.44 s) | 10.0 s |
| Process force-stopped as the transfer began, then relaunched | 11.2–14.4 s |
| Job resumed at downloading; fresh probe (54 ms), model already installed | 15.0 s |
| Audio transferred (6.31 s) | 21.5 s |
| Decoded (8.30 s); writing starts | 29.8 s |

The restart cost about 5 s; without it writing would have started at about
25 s. The app was then sent to the home screen and the job continued under its
foreground-service notice.

The ready notice came 3,582 s (59.7 minutes) after the press, with the app on
the home screen throughout. Writing and translation took 3,552 s of that; for
the first 20 minutes the measurement runs above shared the phone. The job ended
`ready` at a recorded 373.1 KRW, and reopening the video showed Korean subtitles
matching the speech at 2, 30 and 58 minutes. As agreed, the old build was not
run end to end: its downloading stage alone took 1,810–1,836 s, and writing and
translation are unchanged, so the same run before this change would have taken
about 30 minutes longer, roughly 90 minutes instead of 60.

## Limits

- One phone and one home Wi-Fi network, in two sessions. The harness measured
  one lecture; the in-app runs used two. Mobile data, other sites and HLS were
  not measured.
- No server without a strong `ETag` or a dated `Last-Modified` was measured.
  Such a server now gets one whole-file request, as before this change, so its
  speed is unchanged.
- The harness runs as the shell user, not inside the app process; the in-app
  run above uses the app itself and agrees with it.
- YouTube's pacing of whole-file requests and its answers to ranges are
  server behaviour that can change.
- Windows uses the same media download code; it was only checked as a
  diagnostic (5.75 s for the same file from the PC), not measured as a
  before/after pair.
