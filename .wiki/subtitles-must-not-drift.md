---
scope: project
severity: contract
triggers: ["밀림", "싱크", "sync", "타이밍", "타임스탬프", "자막.*(늦|빠르|어긋|맞)", "자막 ?정렬"]
slots: []
sources: [README.md]
links: [translate-text-not-audio, project]
---

# Subtitles stay locked to the audio's timestamps — 밀림 현상 is a defect, not a tuning issue

Rule. Every Korean subtitle line carries the start and end time of the
source speech it translates, taken from the transcription stage, and the
overlay shows it by those timestamps against the video's playback position.
Translation must not reflow, merge or split segments in a way that loses
their source timing. A change to segmentation, batching or translation
prompts is checked for drift over a full-length lecture, not a short clip.

Why. `README.md` names drift (밀림 현상) as unacceptable; the use case is long
technical lectures, where a small per-segment offset accumulates.

What goes wrong. Subtitles that start in sync fall seconds behind by the end
of an hour-long talk, and the product is unusable for its main purpose.
