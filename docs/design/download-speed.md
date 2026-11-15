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
  of the first piece. A piece that drops is asked for again from the byte it
  reached; three failures in a row fail the attempt, which the worker then
  retries from the start as before. A server that answers the first range with
  200 sends the whole file in that one response, read as before. One request
  is open at a time and memory stays at a 64 KiB buffer.
- The worker's first attempt takes the source that job creation (P6) probed
  moments before, instead of probing it again. The hand-off is in memory and
  keyed by job; a retry, or a job resumed after the process restarts, probes
  afresh with the same URL policy and DRM checks.
- P7 reports `preparing_model: true` while the downloading stage fetches the
  speech model. The overlay shows `음성 인식 모델 준비 중(처음 한 번)` and the
  first screen `음성 인식 모델 준비 중` instead of the audio download.
- The YouTube fallback extractor now skips non-default (dubbed) audio tracks,
  as the overlay's browser-session path already did. Both still choose the
  highest-bitrate AAC stream (itag 140 here), so the audio given to the speech
  model is unchanged.
- Each downloading step logs a `timing job` line (on Android to logcat, tag
  `vto`).

Not changed: HLS segments are still fetched one after another, unranged; the
measured slow path was YouTube's direct file. Parallel ranges were not
adopted: on the phone four parallel 10 MiB ranges took 3.85 s against 1.15 s
for one (10.9 MB/s against 9.1 MB/s), so one connection already uses nearly
all of the link.

## Conditions

| | |
| --- | --- |
| Phone | Samsung Galaxy S21+ `SM-G996N`, Android 15, connected by USB debugging. The selected test device was called an S21 Ultra; adb reports this model. |
| Network | Home Wi-Fi, 650 Mbps link, RSSI −41 dBm; no IPv6 route. The PC on the same network shares the public address the signed URL is bound to. |
| Video | "[1hr Talk] Intro to Large Language Models", `zjkBMFhNj_g`, 59:48. Audio itag 140, AAC-LC 128 kbps, 58,064,373 bytes. |
| Source URL | Issued by the app's own YouTube WebView session through the overlay's player request (`VISIONOS` client, 190 ms), the URL the overlay sends as P6's hint. Every run below used the same URL. |
| Baseline code | `678515b` plus the timing log lines only. The app installed beforehand was a 0.1.7-labelled debug build with an unoptimized 255 MB core; all measurements used optimized cores. |
| Harness | `src-tauri/examples/download_timing.rs`, cross-built for `aarch64-linux-android` and run under `adb shell`. It calls the app's own probe, download and decode functions with the same portable HTTP client and rustls as the app. Without `--reuse` it probes twice as the baseline worker did. |

## Results

Times are on the phone. "Writing starts" is probes plus transfer plus decode,
the point at which on-device transcription begins; the model fetch is added on
first use only.

| Step | Baseline 1 | Baseline 2 | Final (5 runs) |
| --- | ---: | ---: | --- |
| Source probes | 7.47 + 14.40 s | 0.26 + 0.09 s | one probe, 0.07–0.37 s |
| Model fetch, first use (59.7 MB) | 6.55 s | not run | 6.8–8.8 s, median 8.7 s |
| Audio transfer (58.06 MB) | 1,796.7 s | 1,798.9 s | 5.67–6.03 s, median 5.80 s |
| Decode | 10.5 s | 10.6 s | 8.8–11.3 s |
| Writing starts, model installed | 1,829 s | 1,810 s | 14.5–17.7 s, median 15.3 s |
| Writing starts, first use | 1,836 s | — | 21.5–25.4 s, median 24.1 s |

Baseline sample 2 and the five final runs ran while the app was transcribing
the same lecture on four threads. Two earlier runs of the same transfer code,
without that load, took 62.2 s and 6.2 s; the 62.2 s run was the only one above
7 s, and it was the slowest of all 8 ranged transfers. Probe times also varied
by network: 7–14 s in the first baseline run, under 0.4 s since.

The agreed targets were a transfer median of at most 60 s over at least five
runs with no run over 3 minutes, and writing starting within 90 s with the
model installed and 2 minutes on first use. All were met.

Every baseline and final run wrote the same file (SHA-256
`a1fadc741fa289c8664bae7c221ffc3ab4b70c6c0a17be41bd9cf9810fd6583c`) and decoded
it to the same 57,403,814 samples at 16 kHz (3,587,738 ms, first sound at
64 ms, PCM SHA-256 `f04512f5…`). The speech model receives exactly the audio it
received before, on the same timeline.

## In the app

The final build (core SHA-256 `d49b6d88…`) was installed over the existing app,
keeping its data. The model was moved aside so the run was a first use.

| Event | Time after the press |
| --- | ---: |
| Job created, source probed once (119 ms) | 0.4 s |
| Overlay shows `음성 인식 모델 준비 중(처음 한 번)` | 1.6 s |
| Model fetched and verified (9.44 s) | 10.0 s |
| Process force-stopped as the transfer began, then relaunched | 11.2–14.4 s |
| Job resumed at downloading; fresh probe (54 ms), model already installed | 15.0 s |
| Audio transferred (6.31 s) | 21.5 s |
| Decoded (8.30 s); writing starts | 29.8 s |

The restart cost about 5 s; without it writing would have started at about
25 s. The app was then sent to the home screen and the job continued under its
foreground-service notice.

END_TO_END_PLACEHOLDER

## Limits

- One phone, one home Wi-Fi network, one lecture. Mobile data, other sites and
  HLS were not measured.
- The harness runs as the shell user, not inside the app process; the in-app
  run above uses the app itself and agrees with it.
- YouTube's pacing of whole-file requests and its answers to ranges are
  server behaviour that can change.
- Windows uses the same media download code; it was only checked as a
  diagnostic (5.75 s for the same file from the PC), not measured as a
  before/after pair.
