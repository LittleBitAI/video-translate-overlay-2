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
cue must still match. Active processing and deletion prevent the repair.
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
