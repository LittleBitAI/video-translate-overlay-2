---
scope: project
severity: preference
triggers: []
domain: ''
title: "Watch the translated videos again in the library and organize them into topic-based playlists."
pr: 4
merged: 2026-11-14
branch: "saved-video-library-and-playlists"
---

# Watch the translated videos again in the library and organize them into topic-based playlists.

What. Added a library next to the address input field. Search for videos by title or address, check if saved subtitles exist, and reopen them within the app. Use completed subtitles from previous versions as they are without repeating transcription or translation. - Location: `src/ui/Watch.svelte`, `src/ui/Library.svelte`, `src-tauri/src/library_store. …

Why. This app transcribes video audio into text, translates it into Korean subtitles, and saves it to the device. Added a library to find and rewatch saved lectures or group them by topic. Previously, one had to know the address again when finding an already translated lecture, and there was no place to collect related lectures. By linking deletion and screen updates, the risk of delayed responses causing deleted subtitles to reappear or triggering unwanted re-translation was also reduced.

Source. PR #4 · `saved-video-library-and-playlists`
