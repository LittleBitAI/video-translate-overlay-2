---
scope: project
severity: contract
triggers: ["밀림", "싱크", "sync", "타이밍", "타임스탬프", "자막.*(늦|빠르|어긋|맞)", "자막 ?정렬"]
slots: []
sources: [README.md, docs/design/backend.md, contract-tests/tests/write_and_timing.rs, tests/overlay/overlay.test.ts]
links: [translate-text-not-audio, repository-gate, project]
---

# Subtitles keep the source speech's timestamps — 밀림 현상 is a defect, not a tuning issue

Rule. Each Korean cue carries the start and end time of the speech it
translates, set at the write (transcription) stage. Translation never
changes them: per assumption A6 in `docs/design/backend.md`, segments go to
Gemini as numbered batches with no timestamps and come back under the same
numbers. The overlay shows cues by the player's current position. Two
designed checks pin this, and they are the evidence for a change to
segmentation, silence trimming, batching or translation:
- T5, `test_cue_times_match_source_after_silence_trim` in
  `contract-tests/tests/write_and_timing.rs`: after repeated long silences,
  a sentence near the end starts within 300 ms of when it was spoken, and
  cues are in `start_ms` order without overlapping.
- T6, `test_overlay_follows_seek_and_pause` in
  `tests/overlay/overlay.test.ts`: after a seek and a pause, the overlay shows
  the cue for the new position.

Both run inside [[repository-gate]].

Why. The original request (quoted in `docs/design/backend.md`) says
subtitles must not drift (밀림 현상), and the design pins it with the
known-timestamp and seek checks above.

What goes wrong. If cue times drift from the source, subtitles fall behind
over a long lecture, and the product fails at its main purpose.
