# Mobile transcription speed and window boundaries

A one-hour lecture took well over an hour to become subtitles on a Galaxy S21+
(SM-G996N, Exynos 2100). The speech model stays the same (`base q5_1`, greedy,
no temperature retries except for a window that loops). This page records what
was measured, how audio is now split before the model hears it, and how the
speech at a cut is kept exactly once. The measurements themselves are in
[the evidence](../evidence/transcription-speed/README.md).

## Result

The same lecture, the app at Home throughout, on the same phone:

| | Agreed target | Before | Now |
|---|---|---|---|
| writing | ≤ 70 min | 60.4 min | 38.7 min |
| press to ready | ≤ 75 min | about 79-84 min | 41.2 min |
| word error rate against the large reference | ≤ 5.40 % | 5.40 % | 5.33 % |
| the same, within 4 s of the old split's 60 cuts through speech | ≤ 13.4 % | 13.43 % | 11.97 % |

The user listened at 20 places (old cuts and technical terms) and judged the
new words as good or better. Writing gained from the dot-product build and
from fuller windows (140 instead of 190). Translation gained from eight
requests in flight (18.0 min to 2.2 min).

## Where the time went

On the representative lecture (59:48, English, technical), the stages were
timed separately on the phone. Each stage logs a `timing` line:
`decode`, `vad`, `model-load`, one `region` line per window, `translate`.
The phone harness (`src-tauri/examples/transcribe_timing.rs`) runs the same
code outside the app.

- Decoding the m4a, finding speech and loading the model take seconds.
- Inference is almost all of the writing stage. The encoder always processes a
  30-second window, whatever its length: a window costs about 5.4 s plus
  0.12 s per second of speech on the phone's big cores. 190 short windows paid
  that fixed cost 190 times.
- Translation sent its 892 one-cue requests one after another, about 1.2 s
  each: 18 minutes spent waiting for replies.
- With the app in the background, Samsung puts the process in its `moderate`
  cpuset, the three little cores. A window then takes about four times as
  long. This is the largest single factor, and it is outside the model.

## Speech engine build

Android's whisper.cpp is built for ARMv8.2 with dot product and half
precision (`GGML_CPU_ARM_ARCH=armv8.2-a+dotprod+fp16` in
`src-tauri/android/whisper.toolchain.cmake`). On the S21+ it wrote the same
600 s 29 % faster on all cores and 15 % faster on the little cores. Its word
error rate against the large reference was 4.54 %, against 4.59 % for the
plain build. Flash attention was slower on every core type and stays off.

Phones with Cortex-A55/A75 or newer cores have these instructions. Phones
built only on Cortex-A53/A73 cores (older and some budget models) do not, and
would crash on the first one. `whisper::cpu_runs_model` therefore checks both
features before the model loads. Without them the job stops with the failure
reason `unsupported_cpu`, and both screens say that this phone's CPU cannot
write speech on the device. A job already written is not affected.

whisper-rs-sys builds whisper.cpp once per target directory and does not see
a changed toolchain file. `scripts/package-release.ps1` therefore cleans it
before every Android release and refuses a `libvto.so` without `sdot`
instructions.

## How audio is split (plan 2)

`audio::speech_windows` replaces the plan-1 split for new jobs:

1. Speech spans are found as before: frame energy against a threshold derived
   from the file, pauses under 800 ms kept inside a span, 200 ms padding.
   Quiet speech is found relative to the file's own level, not to a fixed
   level.
2. Spans are packed into windows of at most 30 s, the model's input. Spans
   join a window while the silence between them is at most 1.5 s. Longer
   silence ends the window, so the model never hears long silence, where it
   invents text.
3. A span longer than what is left of the window is cut. The cut is the
   latest 200 ms pause (every frame below the threshold) in the last 10 s
   before the limit. Such a cut needs no overlap.
4. Where no one pauses for 10 s, the cut goes in the quietest 200 ms there,
   and the two windows share 3 s of audio around it (`OVERLAP`).

Every window keeps its offset on the source timeline. A cue's time is the
window's start plus the model's time inside the window, clamped to the window.

## How a cut through speech is reconciled

Before recognizing a window, both platforms now apply an on-device speech
detector so music admitted by volume detection cannot become captions. Speech
portions retain their source offsets and short pauses; the original window
plan and the reconciliation below still apply. The [music-caption repair
record](../evidence/music-captions/README.md) describes the detector, the MIT
opening regression and the explicit saved-version repair.

Both windows hear the 3 s overlap, so both may write the words in it, a word
cut at an edge may come back as a fragment, and a sentence may be split
between the two (`src-tauri/src/stitch.rs`).

- The model gives word times (token timestamps; decoding is unchanged).
- The earlier window holds back its cues that reach into the overlap.
- When the next window is written, the held cues and the new window's head are
  aligned word by word. The words must match in text (letters and digits, case
  ignored) and in time (1 s for a run, 0.5 s for a single word). The longest
  such run wins; ties go to the smaller time disagreement, then to the run
  nearest the overlap's middle.
- The output switches from the earlier window to the later one at the matched
  word nearest the middle. Each word is therefore kept from exactly one window.
  Repetition the speaker actually said ("very very very") stays, since a
  repeated word is matched against its own position in time.
- With no matching word, the switch is at the overlap's middle. Where one side
  heard nothing in the overlap, the other side's words are kept.
- A sentence that the cut split becomes one cue again.
- Committed cues are ordered and never overlap: each starts at or after the
  previous one's end.

### How long the overlap is

The lecture has a sustained pause near every limit, so plan 2 overlaps none
of its windows. To tune the overlap, the PC harness decoded it the phone's
way with every cut inside a span forced through speech
(`transcribe_timing --forced-cuts --overlap-ms N`), as if nobody paused:
71 cuts. Errors were counted from one alignment of the whole transcript with
the large reference, within 2 s of a cut.

| Overlap | Windows | Errors within 2 s of the 71 cuts | Overall, without 36:00-37:00 |
|---|---|---|---|
| none (hard cut at the quietest 200 ms) | 149 | 41 | 5.26 % |
| 1 s | 149 | 85 | 5.55 % |
| 2 s | 151 | 51 | 5.40 % |
| 3 s | 154 | 41 | 5.36 % |
| 4 s | 155 | 40 | 5.46 % |

3 s matches the best result at the cuts, and needs less audio than 4 s.
A short overlap is worse than none. Its few shared words rarely give a run to
match, and the earlier window's last second is where the model drops words
or stacks several on its last instant. At 36:00 the speaker says "chunk
chunk chunk…" and the model loops on it in every variant; that minute is
left out of the overall column, where one loop decides the order.

Two changes to the reconciliation were tried on the same decoded windows
and dropped: refusing single-word matches (fewer wrong matches, but more
words kept twice), and switching sides 0.25-0.5 s into the overlap instead
of at its middle (no measurable difference).

## When the model loops

At 36:52 the speaker says "chunk" seven times, to a beat. The phone's
decoding (greedy, no temperature retries) looped there on plan 2's window and
wrote "chon" 104 times over 22 s. That lost the speech after it, and the
cue's translation ran past the 120 s timeout five times. Gemini billed those
requests, but the job could never count them, and it stopped at 99 %.

- A window whose segment repeats a unit holding a letter more than 8 times
  in a row is decoded once more, with whisper's temperature fallback
  (`temperature_inc` 0.2). It retries only the parts that fail its
  repetition and confidence checks. Every other window is decoded as before.
  whisper.cpp seeds its sampling on every call, so a resumed job gets the
  same text.
- Whatever still loops is cut to 8 repeats when a cue is committed
  (`stitch::collapse_repeats`). Digits are never cut, and real repetition
  ("very very very", "chunk" seven times) stays.
- Each translation reply stops at 4,096 output tokens. A reply therefore
  comes back with its usage well inside the timeout, and its cost is counted.
  The glossary, the longest reply, is about 1,000 tokens.

## Resume

A job records its plan (`job.plan`). Jobs created before this change have no
plan column and read as plan 1. They resume on their original windows with the
original split, never re-planned mid-job. A window's cues and the cues it holds
for the next window are saved in one transaction (`job.held`). A job killed
after a window that ends in an overlap resumes at the next window with exactly
what it held, and writes the same cues as an uninterrupted run.

## Translation

Each cue is still its own request, with the glossary and only the cue before
it as context. Up to 8 requests wait for replies at once. A request alone
starts while anything is left under the job's limit, as when they went one at
a time. A request beside others starts only if the amount spent, plus every
request in flight costed at the most it can bill, stays under the limit. The
most a request can bill is its prompt counted as one token per byte (no token
is shorter) and the 4,096-token reply cap, or the dearest request so far if
that is more. So only a request that started alone can cross the limit, by
its own cost, as before. Costing in-flight requests at the dearest one seen
was not enough: a cheap first reply let eight dear ones start together.
The store is accessed under one lock, because the worker's connection holds one
transaction at a time.

## Device limits

- With the app at Home, Samsung keeps the process on the three little cores
  (`/moderate`, CPUs 0-2) and in the `background` CPU group, whatever its
  foreground-service type and screen state. A window there takes 2-3 times
  as long as in front. Another app waking up (a payment app, a messenger)
  can starve it: single windows took 193-243 s instead of 15 s. The app does
  not keep the screen on or ask to stay in front. The user chose that.
- Background apps on this phone also compete with the job. T-money spinning a
  core made windows take 75-223 s until it was force-stopped.
- Writing on the phone needs ARMv8.2 dot product and half precision
  (`unsupported_cpu` otherwise).
- A window that loops costs a second decoding: 71.7 s instead of about 15 s
  in the background for the lecture's one such window.
- Only the S21+ (Exynos 2100) was measured. Phones with other big/little
  layouts and other vendors' background policies will differ.
