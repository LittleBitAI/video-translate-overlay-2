---
scope: project
severity: preference
triggers: []
domain: ''
title: "The preparation time for mobile subtitles for a one-hour lecture has been reduced from about 80 minutes to 41 minutes."
pr: 8
merged: 2026-11-17
branch: "improve-mobile-transcription-speed-and-boundaries"
---

# The preparation time for mobile subtitles for a one-hour lecture has been reduced from about 80 minutes to 41 minutes.

What. The method of dividing audio into windows has been changed. Speech separated by short pauses is grouped into a single window close to 30 seconds, and windows are cut at silences exceeding 1.5 seconds. Speech exceeding 30 seconds is cut at the latest 0.2-second pause within the last 10 seconds. Only when there is no pause at all is it cut at the quietest point, and the preceding and following windows are made to listen to the 3 seconds surrounding it together. The number of windows for one lecture has been reduced from 190 to 140. The fixed cost incurred per window has decreased by that much, and segments spoken quietly are still found. …

Why. When the detection button is pressed in the video, the mobile phone receives the audio, transcribes the speech into text within the device, and translates that text into Korean subtitles. The task continues even if the app is moved to the home screen. The speech-to-text model listens to audio segments (windows) of up to 30 seconds in length, one at a time. Subtitles for a one-hour lecture took well over an hour to appear, and the previous method would cut where speech continued, causing words at that point to be missed or fabricated. If the transcription repeated the same sound, the translation request for that subtitle would exceed the time limit. Then, while a charge would be incurred, the app could not calculate the cost, and the task would stop at 99%.

Source. PR #8 · `improve-mobile-transcription-speed-and-boundaries`
