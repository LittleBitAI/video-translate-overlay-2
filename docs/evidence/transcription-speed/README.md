# Mobile transcription speed: execution evidence

Phone: Galaxy S21+ (SM-G996N, Exynos 2100; 1 X1, 3 A78, 4 A55 cores),
Android 15 (SDK 35), on the PC's USB; the battery level is given with each run.
Lecture: YouTube `zjkBMFhNj_g`,
"[1hr Talk] Intro to Large Language Models", 59:48 (3,587,738 ms, English,
technical). Its audio file (m4a) has SHA-256 `a1fadc74…` and is the same file
for every run. Model: `ggml-base-q5_1.bin` (SHA-256 `422f1ae4…`), greedy, no
temperature retries. Thermal status and temperatures were polled every 30 s
from the host (`dumpsys thermalservice`).

## What is published and how to check it

The raw phone logs, the app's store copies and the transcripts stay outside
Git. [`data/`](data/) holds what the tables below come from:

- `data/observations/`: the app's own timing lines from each phone log (the
  `I/vto` lines with `timing job`, `created` and `ready`; other apps' lines
  are left out), the phone harness's timing lines, and the thermal and
  cpuset polls exactly as recorded.
- `data/transcripts/`: every transcript the evidence scores: the large
  reference, each build's cues, the restart snapshots, the forced-cut runs
  and the cold/warm passes. Window and cue times are kept as recorded. Each
  word is replaced by a keyed hash of its normalized form
  (`scripts/sanitize.py`). Equal words keep equal hashes, so the word error
  rates come out the same. The key was random and is gone: the lecture is
  someone else's talk, and its text cannot be rebuilt from these files.

`python -I scripts/reproduce.py` recomputes every scored claim from those
files and fails on any mismatch: the word error rates, the overlap table,
both restart checks, the cold and warm passes, and the final run's stage
times, window counts, thermal and cpuset samples. On the files committed
with it, it prints 38 `ok` lines and no `FAIL`. `scripts/` also holds the
scripts that made the runs (`start-run.sh`, `until-ready.sh`,
`restart.sh`, `writing-only.sh`, `thermal-poll.sh`, `reference.sh`,
`cues-from-db.py`, `spot.py`, with the press script `press-confirm.js` and
`cdp.mjs`). Local paths and the signed-in account's folder are taken from
environment variables there.

### Build identities

SHA-256 of what ran:

| What | Source | SHA-256 |
|---|---|---|
| APK, baseline in the app | eb9b394 | `69131e8ed0ca8de4b64532964378c2d3b33488b3db5e290ba1f92fc7a12330fe` |
| APK, first final build (stopped) | b2c8b5e | `2cf038171d9bf02297ac15ddfa0d0c50a227babcf4e0abd1da121de0dfad62a5` |
| APK, final in the app and both restart checks | 41f5014 | `1f1523b41fb97c0ce08adae676b18636166d2fc0cbad5206d9779e34e96989b7` |
| its `libvto.so` (650 `sdot` instructions) | 41f5014 | `6e432e904edaa417139e3f9693c58319c6d1b3fdadcbb8cdaa020ce668a7156c` |
| APK installed before and after the task | c632ca1 | `c4783432ddcbc4cd9a6012960b303d81bc03b5acdd7b58c690fabd278cebb03c` |
| phone harness, cold/warm passes (650 `sdot`) | 41f5014 | `b9a7f0d73b1050a9d905859214d3d0313b51395a7dabba31df204c437d740434` |
| PC harness, looping-window check (phone decoding) | 41f5014 | `dc89cc01ef3bed2e04a8a738ab463f3e6e3acf417982f024d002df5f45dcc46d` |
| lecture audio (m4a) | — | `a1fadc741fa289c8664bae7c221ffc3ab4b70c6c0a17be41bd9cf9810fd6583c` |
| phone model `ggml-base-q5_1.bin` | — | `422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898` |
| reference model `ggml-large-v3-turbo-q5_0.bin` | — | `394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2` |

### From the measured build to the current branch

The runs used 41f5014. Later commits change three things, and none of them
changes what was measured:

- The window planner checks the room left in a window before subtracting
  (a pause just past a full window made it wrap around). For this lecture,
  the current planner gives the same 140 windows, start and end, as the
  phone's log of the final run.
- A translation request now starts beside others only if, with every request
  in flight costed at the most it could bill, the limit still holds. The most
  one request of the final run could bill (its glossary, its two longest cues,
  the system text, and 4,096 output tokens) is 10.48 KRW. The run spent at
  most 366.58 KRW, and 366.58 + 8 × 10.48 = 450.43 KRW, under its
  996.59 KRW limit. So all eight requests were admitted exactly as before.
- The opt-in whisper benchmark compiles again. It is a test only.

## Baseline (commit eb9b394, plan 1)

### Phone harness, cold, all cores (`adb shell`, top-level cpuset)

| Stage | Wall time |
|---|---|
| decode (m4a to 16 kHz PCM) | 4,764 ms |
| speech regions (plan 1) | 45 ms, 190 regions, 3,496,678 ms of speech |
| model load | 91 ms |
| inference, 190 windows | 1,434,085 ms (23.9 min) |

Thermal: status 2 (moderate) for 35 of 46 samples; AP peaked at 51.2 °C and
skin at 40.9 °C.

### In the app (APK at eb9b394, one paid run)

The app was sent to Home after the detect press. At window 28 (12:42:59) an
external launch brought the app back to the front. The run is therefore mixed:
windows 0-27 ran in the background, most later windows in front.

| Stage | Wall time |
|---|---|
| audio transfer | 6,472 ms (58 MB) |
| decode | 10,044 ms |
| speech regions | 209 ms, 190 regions |
| model load | 486 ms |
| writing, windows 0-27, background | about 30 s per window |
| writing, later windows, mostly in front | 5-9 s per window |
| writing, all 190 windows | 36 min 7 s |
| glossary | 2,653 ms |
| translation, 892 one-cue requests one at a time | 1,080,431 ms (18.0 min) |
| press to ready | about 54 min |

Cost: 391.91 KRW (878,017 input and 33,851 output tokens). Thermal: up to
status 3 (severe); AP peaked at 53.0 °C and skin at 41.9 °C.

A backgrounded app is in Samsung's `moderate` cpuset (CPUs 0-2, A55 cores).
It has the `CPUSET_SP_MODERATE` task profile and scheduling group 5, whatever
its foreground service. In front it is in `top-app` (CPUs 0-7). With every
window in the background, the writing stage would take about 190 × 30 s,
roughly 95 minutes.

### In the app, background only (APK at eb9b394, writing only, no cost)

The first in-app run was mixed, so the baseline was repeated with the app
at Home throughout. Before every phone run from here on, T-money
(`com.lgt.tmoney`) was force-stopped and the phone given a minute. On a
first attempt it spun a core in the background, and windows took 75-223 s.
A watcher turned the phone's network off three windows before the end and
stopped the app after the last window, before the glossary request.

| Stage | Wall time |
|---|---|
| press to audio | 306,768 ms (58 MB; 6,472 ms in the morning run) |
| decode | 22,920 ms |
| speech regions | 178 ms, 190 regions |
| model load | 394 ms |
| writing, 190 windows | 3,623,989 ms wall (60.4 min; 3,621,330 ms in the model); median window 16.3 s, 11 over 30 s |

The app's cpuset read `/moderate` (CPUs 0-2) and its CPU group
`background` whenever it was sampled during this run. Those samples were not
kept as a file, unlike the final run's. Window 140 took 243 s when another app (Toss) started; in the
harness the same window takes 7.3 s. Thermal: status 0 for 123 of 127
samples, at most 1; AP peaked at 42.4 °C, skin at 38.0 °C; battery 41 % to
37 % on USB.

Its 892 cues are identical, in text and time, to the harness baseline's, so
its accuracy is the harness's: 5.40 % overall, 13.43 % at its 60 cuts.
Writing plus the morning's translation time (18.0 min one at a time) puts
press to ready at about 79 min with the morning's audio transfer, and 84 min
with this run's.

## Final (commit 41f5014, plan 2)

### In the app, background, one paid run

Same conditions as the background baseline: data restored and checked,
T-money force-stopped, a minute's settling, detect pressed at 20:41:26, the
app at Home throughout. Battery 66-67 % on USB.

| Stage | Wall time |
|---|---|
| press to audio | 11,414 ms transfer (58 MB) |
| decode | 4,178 ms |
| speech regions (plan 2) | 48 ms, 140 windows, 3,532,998 ms of speech |
| model load | 129 ms |
| writing, 140 windows | 2,322,092 ms (38.7 min); median window 14.3 s, 2 over 30 s |
| glossary | 2,714 ms |
| translation, 856 one-cue requests, 8 in flight | 132,572 ms (2.2 min), none retried |
| press to ready (notification) | 2,473,366 ms (41.2 min) |

Window 85 (36:45-37:14) was decoded twice, because its first decoding looped
(see the design page): 71.7 s instead of about 15 s. Window 1 took 33.5 s,
the first window after Home. 40 of 41 cpuset samples read `/moderate`; the
first, taken at the press, read `/foreground`. The screen stayed on. Thermal:
status 0 in all 82 samples; AP peaked at 40.0 °C and skin at 33.8 °C.

Cost: 366.58 KRW (768,891 input and 40,393 output tokens), against
391.91 KRW for the baseline's translation. The 856 cues are ordered and none
overlaps the next.

### Against the baseline and the agreed targets

| | Target | Baseline (eb9b394, background) | Final (41f5014, background) |
|---|---|---|---|
| writing | ≤ 70 min | 60.4 min | 38.7 min (−36 %) |
| translation | — | 18.0 min | 2.2 min |
| press to ready | ≤ 75 min | about 79-84 min | 41.2 min |
| word error rate, overall | ≤ 5.40 % | 5.40 % | 5.33 % |
| word error rate at plan 1's 60 cuts | ≤ 13.4 % | 13.43 % | 11.97 % |

The baseline's press-to-ready adds the morning's translation time to the
background run's writing, which stopped before translation.

The user listened at the 20 agreed places
(10 of plan 1's cuts through speech, 10 technical terms; the sheet comes from
`artifacts/tx-speed/scripts/spot.py`). Their judgement: the final build's words
are as good as the old build's or better. Within 4 s of those places the
final build had 73 word errors against the reference, the old build 84.

### Phone harness, cold and warm (first 600 s, 24 windows, all cores)

| Pass | Model load | Inference | Thermal at start |
|---|---|---|---|
| 1, cold (fresh process) | 133 ms | 105,207 ms | status 0, AP 32.0 °C |
| 2, warm (model loaded) | — | 119,260 ms | status 0, AP 45.0 °C |

Decoding took 4,224 ms and the speech regions 42 ms, once for both passes.
Both passes wrote the same 158 cues. Warm saves only the model load. The
second pass ran slower on a phone that was still hot from the first. The
baseline's dotprod-free build took 223.4 s for the same 600 s of plan 1
(kernel table below).

### Restart recovery (writing stage, no cost)

`artifacts/tx-speed/scripts/restart.sh` force-stops the app once a given
window is written, relaunches it, sends it Home, and copies the store at the
kill and at the end.

| Job | Killed after | Resumed as | Result |
|---|---|---|---|
| started by the baseline APK (plan 1), resumed by the final APK | window 8 | `plan=1 regions=190 from=9` | the 44 cues at the kill unchanged; 93 at window 16, ordered, none overlapping |
| started and resumed by the final APK (plan 2) | window 8 | `plan=2 regions=140 from=9` | the 64 cues at the kill unchanged; the 118 at window 16 identical to the uninterrupted paid run's |

The 44 cues the baseline build wrote are its harness cues exactly. Of the
49 the final build wrote after resuming the old job, 5 differ from the
baseline's: one word on the other side of a cue boundary, and punctuation.
This is the dotprod build's own difference.

### The build before (commit b2c8b5e), one paid run, stopped

The first final build's run wrote 140 windows in 3,108,715 ms (51.8 min).
Window 66 took 368 s while the phone's USB connection dropped (18:46-18:53),
and window 2 took 193 s. It translated 851 of 852 cues in about 3 minutes
(370.19 KRW). The last cue (36:52-37:14) was 485 characters of
"chonchonchon…": the model had looped on the speaker's "chunk chunk chunk"
and lost the 20 s of speech after it. Its translation ran past the 120 s
timeout five times. The app was force-stopped at 99 % so that more
uncounted requests would not be billed. The five timed-out requests are not
in the 370.19 KRW: the app never received their usage. Its word error rate
was 5.79 % overall (minutes 36-37 alone had 39 more errors than the
baseline) and 11.97 % at plan 1's cuts. The fixes are in the design page:
the looping window is decoded again, a loop that remains is cut to 8
repeats, and a reply stops at 4,096 output tokens.

## Accuracy reference and how it is scored

The reference is the PC's model, `large-v3-turbo q5_0` with the PC's decoding
(beam 5), on plan 2's 140 windows. It ran on an RTX 3080 (whisper.cpp built
with CUDA 12.4, `CMAKE_CUDA_ARCHITECTURES=86`) in 324,560 ms and wrote 1,073
cues. It stands in for a human transcript. Its own errors count against
every build alike.

`artifacts/tx-speed/scripts/wer.py` computes the word error rate: word-level
edit distance over lowercased words without punctuation, divided by the
reference's 11,720 words. "At cuts" counts only words whose time (interpolated
across its cue) is within 4 s of a place where one window ends and the next
begins. `--cuts-from` scores a transcript at another run's cuts, so plan 2 is
judged at the 60 places plan 1 cut through speech.

| Transcript | Overall | At plan 1's 60 cuts | At its own cuts |
|---|---|---|---|
| phone harness, plan 1 (baseline) | 5.40 % | 13.43 % | 13.43 % (60) |
| PC, phone decoding, plan 1 | 6.19 % | 13.30 % | — |
| PC, phone decoding, plan 2 | 5.85 % | 12.16 % | 11.26 % (65) |
| PC, phone decoding, plan 2, looping window decoded again | 5.39 % | 12.16 % | — |
| phone in the app, b2c8b5e (loop not decoded again) | 5.79 % | 11.97 % | — |
| phone in the app, final 41f5014 | 5.33 % | 11.97 % | — |

Plan 1 made up words where it cut through speech: "is a bit of a problem" at
10:09 and "to do a remarkable, accuracy." at 26:09, where the speaker said
neither. Plan 2 cuts at pauses there and writes what was said.

## Kernel variants (phone harness, first 600 s, 30 windows, 597 s of speech)

Each run was cold, in turn, with 60 s between runs (`variants.sh`). "Little
cores" means `taskset 7` (CPUs 0-2), the cores a backgrounded app gets. Word
differences are word-level edit distance against the plain build's 2,001
words.

| Build | Cores | Inference | Thermal status at start | Words different from plain |
|---|---|---|---|---|
| plain armv8.0 (eb9b394) | all | 223.4 s | 0 | — |
| new source, armv8.0 (word times on) | all | 228.5 s | 0 | 0 |
| flash attention | all | 263.3 s | 1 | 0.8 % |
| armv8.2 dotprod + fp16 | all | 158.8 s | 1 | 1.2 % |
| armv8.2 dotprod + fp16, flash attention | all | 170.0 s | 1 | 1.1 % |
| plain armv8.0 | little | 666.7 s | 2 | 0 |
| armv8.2 dotprod + fp16, flash attention, 3 threads | little | 1,087.0 s | 0 | 1.1 % |
| armv8.2 dotprod + fp16 | little | 566.7 s | 0 | 1.2 % |

- Word times (token timestamps) leave the text unchanged and cost about 2 %.
- Flash attention is slower on every core type, and much slower on the little
  cores.
- The dotprod build is 29 % faster on the big cores and 15 % on the little
  ones. With fp16 arithmetic on, it also changes about 1 % of the words.
- On the little cores the plain build is 3.0 times slower than on all cores.

Thermal: the variant runs reached status 2 with AP at most 52.1 °C.

## Background scheduling (in-app, writing stage)

Read from `/proc/<pid>/cpuset` and `dumpsys activity` while a job was writing,
with the app sent to Home:

| Foreground service type | Screen | cpuset | CPUs | Scheduling group | Seconds per window |
|---|---|---|---|---|---|
| `specialUse` (current) | on | `/moderate` | 0-2 | 5 | about 30 |
| `mediaProcessing` (trial build) | on | `/moderate` | 0-2 | 5 | 34-38 |
| `mediaProcessing` (trial build) | off (Dozing, 3 min) | `/moderate` | 0-2 | 5 | 25-38 |
| any, app in front | on | `/top-app` | 0-7 | — | 5-9 |

During a network-bound stage (translation) the same `mediaProcessing`
process read `/foreground` (CPUs 0-2, 4-7) 15 s after Home. While writing, it
is in `/moderate` within 20 s. Samsung appears to place a CPU-heavy
background process on the little cores, whatever its foreground-service type
and screen state. The trial build was not kept: `mediaProcessing` brings
Android's six-hours-a-day limit and no gain. One window that spanned turning
the screen off and on and the lock screen took 282 s. It was not repeated in
3 minutes with the screen off.
