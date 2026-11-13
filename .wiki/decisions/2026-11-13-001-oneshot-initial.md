---
scope: project
severity: preference
triggers: []
domain: ''
title: "I created the first app that adds Korean subtitles to foreign language lectures with a single video detection."
pr: 1
merged: 2026-11-13
branch: "oneshot-initial"
---

# I created the first app that adds Korean subtitles to foreign language lectures with a single video detection.

What. When video detection is pressed, three steps run in sequence: receiving audio, transcribing with an on-device speech recognition model, and translation. Before translation, a list of technical terms from the lecture is created to ensure the same translation is used for all sentences. Subtitle timing is synced to the original video's timeline. Only one task runs at a time, and if the app is turned off and on, it resumes from where the transcription left off. - Location: `src-tauri/src/pipeline.rs`, `src-tauri/src/whisper.rs`, `src-tauri/src/gemini. …

Why. This is an app for people who want to watch technical lectures in foreign languages with Korean subtitles. When you press the video detection button on the video, the app receives the video's audio, transcribes the speech into text on the device, and translates that text to create subtitles that appear in sync with the speech. YouTube's automatic subtitles are inaccurate for technical lectures, and having a translation model convert audio directly to subtitles is too expensive. Therefore, transcription is done on-device, and only the text is sent to an inexpensive translation model, lowering the cost to around 1,000 KRW per hour. The screen colors follow the charcoal theater color scheme (dark background, blue main buttons, yellow warnings) chosen by the owner during the review.

Source. PR #1 · `oneshot-initial`
