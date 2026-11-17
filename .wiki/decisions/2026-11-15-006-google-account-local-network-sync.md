---
scope: project
severity: preference
triggers: []
domain: ''
title: "Automatically share the library and playback position between Windows and Android on the same network."
pr: 6
merged: 2026-11-15
branch: "google-account-local-network-sync"
---

# Automatically share the library and playback position between Windows and Android on the same network.

What. Provides Google app login separate from YouTube login and separates the library for each account. Existing device libraries are copied after verification, and the original is maintained. Device login is maintained for up to 30 days, and it is noted in the settings that it is not immediately revoked even if the Google password is changed elsewhere, along with information on personal information processing. - Location: `src/ui/GoogleAccount.svelte`, `src/ui/Library.svelte`, `src-tauri/src/account. …

Why. This app stores Korean subtitles created from foreign language videos and organizes them into playlists. It adds device connection by Google account so that saved subtitles and playlists can be used continuously even when changing devices. It reduces the inconvenience of re-translating the same video or managing playlists separately when moving devices. By leaving a copy on the device without a separate synchronization server, it prevents leakage to other accounts and the reappearance of deleted data.

Source. PR #6 · `google-account-local-network-sync`
