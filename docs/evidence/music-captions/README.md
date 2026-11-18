# Music captions and the saved MIT lecture

The user supplied a Windows screenshot of `MIT Introduction to Deep Learning
(2023) | 6.S191`, YouTube `QDX-1M5Nj7s`. This corrects the original task's
reference to `zjkBMFhNj_g`. The LLM lecture's opening contains speech and is
outside this repair.

## Reproduction

The selected MIT response is
`t/QDX-1M5Nj7s/dae8e80e2206305ffe0d3e679af5df6474dcfa75081c8045d1ab6a6cda4d8973`.
Its source cue 0 runs from 1,180 to 5,800 ms and begins with a musical note
inside a bracket. The source repeats a background-music description and ends
mid-description. The Korean cue repeats the same description, exactly as in
the screenshot. The defect therefore precedes translation and overlay display.
The next cue is speech at 9,100–18,100 ms. There are 755 saved cues and a
recorded cumulative cost of 276.2938750000004 KRW.

The retained [download run](../../design/download-speed.md) identifies the
generating platform as Android on a Galaxy S21+, source `c632ca1`, APK SHA-256
`c4783432ddcbc4cd9a6012960b303d81bc03b5acdd7b58c690fabd278cebb03c`.
Its ready receipt has the same video, cost, cue count and first-cue start.
The response was later synchronized to Windows. The installed Windows binary
has SHA-256
`2cc473e758178127b7f28ce55e3f2587bd8fe65672f300f949ce4e3f74f504df`.
The user screenshot has SHA-256
`89fa4d790f8eec4282477ff32e5b9a6e78cecc51f055b3242263df5df3fedf49`.

The original build already suppressed Whisper's non-speech tokens. Volume
detection nevertheless admitted the music, and the recognizer emitted its
description as ordinary text. Repetition limiting alone would leave an
unwanted caption and cannot fix this cause.

## Generation

The same Android base and desktop large-v3-turbo speech models remain in use.
The existing whisper-rs dependency supplies the on-device Silero 6.2 speech
detector. Its auxiliary model is downloaded with the existing checked,
retrying model downloader: 885,098 bytes, SHA-256
`2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987`.
No audio goes to Gemini.

Before each existing model window is decoded, the detector finds speech with
its default 0.5 probability threshold, a 100 ms minimum utterance and 300 ms
edge padding. Quiet input is scaled for detection only. Detected spans with
gaps up to the existing 1.5-second packing allowance stay together. Each
remaining portion is decoded separately and receives its original window
offset, including its word times. Audio is never concatenated. Existing
outer-window plans, overlap stitching and repetition recovery remain in use.

Filtering decoded words was rejected: on this opening the recognizer placed
some real introductory words in the music's timestamps. Preventing the model
from hearing a long musical lead-in avoids discarding those words afterwards.

## Targeted repair

`repair_saved_music` is a tools-only example; it is not bundled into installers.
It handles one explicitly named cue from one selected account-library version.
It checks that cue's source audio, including 300 ms of surrounding audio, and
refuses to remove a cue when speech is detected. The audio file must start at
the video's source time zero. The operator obtains it from the named video;
the command cannot establish a downloaded file's video identity on its own.

```powershell
cargo build -p video-translate-overlay --features whisper --example repair_saved_music
target/debug/examples/repair_saved_music.exe <database> QDX-1M5Nj7s <selected_version> 0 <source_audio_at_0> <models>
```

Without `--apply`, this prints a preview. Adding `--apply` uses one SQLite
transaction to retain the old response, remove the confirmed non-speech source
cue from a new response, project that response through the existing ready-job
and saved-selection mechanism, and commit. The selected identity and projected
cue must still match the exact cue captured before audio inspection, including
its interval, source text and Korean text. Selection and cue are captured in
one database snapshot. Active processing and deletion prevent the repair.
Failure or interruption rolls back publication and projection together.

The replacement reuses every remaining source and translation byte and their
original times. It carries the maximum cumulative cost from all retained
responses and jobs. It makes no paid calls, requires no Gemini key, and does
not change playlists, playback position, unrelated videos or other accounts.
Its version synchronizes normally; an old response arriving again cannot
unexpectedly select itself over the newer repair. Explicit selection of an
older response remains available.

## Verification status

Local input audio, store snapshots and machine logs are retained under ignored
`artifacts/music-opening/`; they are not public library exports. The 35-second
MIT opening was downloaded from the named public video and decoded to 16 kHz
mono. The WAV SHA-256 is
`df962fa26e63bf34842203f889ecf7040a85e155010eae9199ae490968605835`.
The real-audio check also uses the previously retained LLM recording's spoken
repetition at 36:52–37:05, without repairing that video.

The exact original music window and its context contain no detected speech.
Both speech models now return no captions for that window and retain the
spoken introduction. The first spoken cue starts at 9,040 ms with the phone's
decoding and 9,280 ms with the desktop model, within the existing 300 ms
tolerance of the saved 9,100 ms speech onset. Quiet speech, speech mixed with
music, genuine rhythmic speech, reused detector state and reopening also pass.

| Focused command | Result |
| --- | --- |
| `cargo test -p video-translate-overlay --features whisper --lib mit_music_and_contrasting_speech -- --ignored --nocapture`, with the three audio/model environment variables named in the test | 1 passed with both real speech models; 84.88 s. |
| `cargo test -p video-translate-overlay --features whisper --lib sync::library::tests -- --nocapture` | 15 passed, including injected write failure, interruption, stale selection, retained costs and old-version selection, sync, other videos, playlists, playback and another account. |
| `cargo test -p video-translate-overlay --features whisper --lib detected_voice_keeps -- --nocapture` | 1 passed: original offsets, short-pause packing and clipping. |
| `cargo test -p video-translate-overlay --features whisper --lib stitch:: -- --nocapture` | 7 passed: overlap, ordered nonoverlapping cues and genuine repetition. |
| `cargo test -p vto-contract-tests --test write_and_timing -- --nocapture`, with `VTO_CONTRACT_HOST` pointing to the rebuilt host | 2 passed, including T5's 300 ms late-speech timing tolerance. |
| `cargo test -p vto-contract-tests --test jobs test_job_advances_in_background_and_resumes -- --nocapture` | 1 passed, including T8's restart/background behavior. |
| `npx vitest run tests/overlay/overlay.test.ts tests/overlay/hidden_mount.test.ts` | 37 passed, including T6's seek/pause and hidden-page return. |
| `node artifacts/music-opening/api-check.mjs` | Passed actual loopback API reads before/after repair and process restart in an isolated library copy; retained all four other saved videos. |
| `python artifacts/music-opening/verify-production.py` | Passed the actual installed-library comparison, reopening through a fresh connection and SQLite integrity check. |

The tools and host were rebuilt from the final implementation with the
`whisper` feature. [Receipts](receipts.json) bind the exact source files, local
tool binaries, audio and sanitized API/production observations. The local
diagnostic scripts and private library snapshots remain ignored. Stand-ins
were used only by the isolated API host; repair used the real audio detector
and made no Gemini requests.

## Actual installed-library repair

The explicit command completed against the installed account library with
`--apply`, retaining the previous response and selecting the new response:
`t/QDX-1M5Nj7s/b46450964281773a3b439fbe164cb5ac0b9aa0fcc8dc77c13c6a240554de8b27`.
There are now 754 cues, beginning with the original speech at 9,100 ms.
Every retained source string, Korean string and start/end time matches the
pre-repair snapshot exactly. The previous response's stored bytes are unchanged.
The cost is still 276.2938750000004 KRW, with zero new paid calls. All video
metadata, playlists, memberships, playback positions and browser state, plus
unrelated jobs, cues and selected responses, match the fresh pre-repair snapshot.

The test cleanup audit retained the existing stitch/repetition, timing,
background/resume, sync/cost and overlay checks. Two intermediate tests of
the abandoned decoded-word filter were folded into the real-audio regression
and voice-offset check; the final pipeline has no decoded-word filter.
The opt-in inference tests still require local audio and model files, so their
ignore reasons remain valid; the new regression was explicitly run above.

On 2026-11-18 the user selected `Playback checks passed (Recommended)` after
being asked to reopen this MIT lecture in Windows, confirm no captions during
0–8 seconds and normal Korean speech captions during 9–20 seconds, check
seek/pause, and restart the app to confirm the repaired selection persists.
This is the user's confirmation of actual Windows playback, not an automated
desktop observation; the session's native UI transport was unavailable.

The server owns the repository gate and final verification, which remain
pending. No installer or release was published, and no physical Android
inference run is claimed for this change.

## Integration with desktop GPU transcription

The task branch merges the exact main commit
`435ff315743ca4731b6c8157c237ae9ef019f961`. Its Windows Vulkan backend,
bundled loader, GPU beam search and CPU greedy fallback remain in place.
Speech detection still precedes recognition, and each speech portion retains
its original source offset. Loop recovery now follows main's greedy-decoding
policy for both phone and CPU fallback, retrying only the affected speech
portion rather than feeding its musical lead-in back to the recognizer.

The earlier execution receipts and Windows playback confirmation describe the
initial implementation. They are historical evidence, not approval of this
integration. Fresh integration checks and source hashes are recorded separately
in the receipts. The server owns subsequent checks and a fresh independent
review; no previous review is carried forward. This merge does not modify the
installed lecture's saved data or publish an installer.

Fresh focused checks on the merged sources:

| Command | Result |
| --- | --- |
| `cargo test -p video-translate-overlay --release --features whisper --lib mit_music_and_contrasting_speech -- --ignored --nocapture` | 1 passed on NVIDIA GeForce RTX 3080. Both speech models left music blank and retained the opening speech. |
| The same command in a fresh process with `VK_DRIVER_FILES` pointing to a nonexistent file | 1 passed; both models logged CPU execution. First cue starts were 9,040 ms for base and 9,280 ms for large, as on the GPU, within 300 ms of the source speech at 9,100 ms. |
| `cargo test -p video-translate-overlay --release --features whisper --lib stitch:: -- --nocapture` | 7 passed: source offsets, overlap reconciliation, ordered nonoverlapping cues and genuine repetition. |
| `cargo test -p video-translate-overlay --release --features whisper --lib detected_voice_keeps -- --nocapture` | 1 passed: nearby speech, original offsets and music-only empty output. |
| `pwsh -NoProfile -File tests/package-release.test.ps1` | Passed 18 refusal cases and both fresh-output cases, with the build boundary stubbed and no candidate mutation. |

The release-profile tests used `CARGO_TARGET_DIR=C:/vtt`, Vulkan SDK 1.4.363.1
and the same pinned audio and models as the earlier regression. The initial
debug build was stopped during slow compiler capability probes; it supplies
no test result. The release-profile runs above completed successfully. These
are correctness checks, not new transcription-speed measurements.

The integration cleanup audit retained the real-audio, voice-offset, stitching
and packaging tests. They guard distinct current behavior; the opt-in audio
tests still need local audio and models. No tests were removed or weakened.

## Review round 1: inspected cue, override and readable evidence

Publication now receives the exact cue that was inspected. Capture reads the
requested selection and cue together; publication compares the captured interval,
source text and Korean text inside its transaction. The regression
`silent_cue_repair_denies_any_change_to_the_inspected_selection_or_cue` denies
version interleavings and a table of changed cue fields without losing speech.
This rule applies at two store boundaries, capture and publication, and the
single repair-command caller now carries the captured cue through detection.

An existing speech-model override no longer disables the separate detector's
provisioning. The detector uses the app's writable model cache, leaving the
override directory unchanged. Preparation, the pending-download indicator and
direct resumed transcription all account for this dependency. The offline
override regression checks first provisioning, reuse and failure propagation.
The real-audio regression also uses a speech-only override directory and a fresh
app cache for each actual model; its provisioner copies the pinned local detector
fixture so these checks make no new external requests.

Three evidence families now have readable observations:

| Evidence | Observed result |
| --- | --- |
| [Actual HTTP requests and assertions](review-one-api.json) | 13 recorded target/library operations and 41 assertions against the real local API. |
| [Actual Windows native-host requests and assertions](review-one-native.json) | 13 recorded target/library IPC operations and 43 assertions in WebView2 157.0.4341.46 on Windows 11 Home, build 10.0.26200, x64. Core and screen both identified `eead0b9989de21c25d683d5260352134d264fb0c`. |
| [Focused command outputs](review-one-checks.log) and [source bindings](review-one-binding.json) | Readable test outputs, commands, model/audio hashes and the relation between the tested sources and this evidence commit. |

Both runtime runs started with separate disposable copies of the original
755-cue MIT response. They ran the rebuilt repair tool, checked the 754 remaining
source/Korean cues byte-for-byte through a separate read-only SQLite observer,
retained the old response and 276.2938750000004 KRW, selected both versions through
the actual API, and stopped and restarted the host to check the repaired selection.
Four other saved responses, playlists, memberships and playback positions remained
unchanged. The packets contain request/response projections and explicit expected
and observed values. Unrelated content stays behind digests; no private library
export, account credentials or browser profile is published.
Auxiliary unrelated-response reads are compared through aggregate digests. The
existing native bridge also reads the account generation before each operation;
these auxiliary reads are not counted among the 13 recorded target operations.

The inspectable producers are
[`music-repair-evidence.mjs`](../../../scripts/verification/music-repair-evidence.mjs)
and its [read-only observer](../../../scripts/verification/music-repair-observer.py).
The HTTP host used stand-ins for processing services; the native host used the
actual debug Tauri/Wry application. Neither run created a translation job or made
a paid call. Native startup update checks used an empty local fixture. Each host
and its owned processes were stopped. The installed user's library was untouched.

These are fresh native-host IPC observations, not new player UI actions or a new
automated playback claim. Native UI discovery reported an unavailable pipe. The
earlier owner-confirmed Windows playback remains explicitly reused: the watch,
overlay, subtitle API and native screen sources are unchanged since that report,
the repaired response is the same, and fresh overlay tests still cover seek and
pause. The GPU and CPU real-audio checks are fresh on the repaired sources.
The original API summaries and integration log hashes remain historical; this
packet supplies their inspectable current replacements. Server-owned checks and
fresh independent review remain pending.

The final cleanup audit retains the binding/interruption/sync tests, detector
provisioning test, real-audio test, synthesized-speech test, stitching, timing,
background/resume and overlay tests. They cover current distinct behaviors.
The opt-in tests still need external model/audio prerequisites; their ignore
reasons remain valid. No tests were removed or weakened in this review repair.
