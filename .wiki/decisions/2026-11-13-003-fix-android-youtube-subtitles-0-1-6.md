---
scope: project
severity: preference
triggers: []
domain: ''
title: "Stabilizing Android YouTube subtitle flow and optimizing transcription"
pr: 3
merged: 2026-11-13
branch: "fix/android-youtube-subtitles-0.1.6"
---

# Stabilizing Android YouTube subtitle flow and optimizing transcription

What. Maintain login cookies for the app WebView, restore video addresses after login, and link account connection/logout in settings. - Create a video detection button even in Trusted Types environments, and pass the signed YouTube audio URL received from the browser session to the Rust task. Maintain address, audio, and DRM checks. - Apply multilingual base q5_1, greedy search, a maximum of 4 threads, and inference buffer reuse for Android speech recognition. Maintain the PC model. …

Why. (the PR body has no reason section; the grounds for this decision were not recorded)

Source. PR #3 · `fix/android-youtube-subtitles-0.1.6`
