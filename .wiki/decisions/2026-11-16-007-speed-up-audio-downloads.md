---
scope: project
severity: contract
triggers: ["tts", "qwen", "음성", "목소리", "합성", "재생"]
domain: tts
title: "By receiving YouTube lecture audio in chunks, the download time for a 1-hour lecture was reduced from 30 minutes to 6 seconds."
pr: 7
merged: 2026-11-16
branch: "speed-up-audio-downloads"
---

# By receiving YouTube lecture audio in chunks, the download time for a 1-hour lecture was reduced from 30 minutes to 6 seconds.

What. Instead of requesting the audio file in its entirety at once, it is received sequentially in 10MiB segments. It verifies whether each segment starts at the byte where the previous segment ended and whether it is the same file as the first segment received (checking if the total length and the file identifier assigned by the server match). If a connection is lost, the segment is resumed from where it left off, and if it fails three consecutive times, the attempt is treated as a failure. For servers that do not support range requests, it is received in its entirety at once as before. Only one connection is used at a time, and memory usage remains constant with a single small buffer. …

Why. When a user presses the video detection button on a video, the app downloads the video's audio, transcribes it into text on the device, and translates it into Korean subtitles. Subtitle processing begins only after the audio download is complete. Until now, it took 30 minutes just to download the audio for a 1-hour lecture on a mobile phone. YouTube sends the entire audio file at only about twice the playback speed (approximately 32KB per second) when requested all at once. When the same file is requested in segments, it is sent at the full Wi-Fi speed. Therefore, even though the mobile phone's network connection is fast, users had to wait 30 minutes before the subtitle work could begin.

Source. PR #7 · `speed-up-audio-downloads`
