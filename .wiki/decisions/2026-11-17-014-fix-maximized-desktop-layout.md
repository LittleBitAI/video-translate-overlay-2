---
scope: project
severity: contract
triggers: ["화면", "vision", "프레임", "스크린", "캡처", "공유"]
domain: vision
title: "Video, library, and settings utilize the wide screen when the window is maximized."
pr: 14
merged: 2026-11-17
branch: "fix-maximized-desktop-layout"
---

# Video, library, and settings utilize the wide screen when the window is maximized.

What. In a wide window, align the menus and the starting point of the main content for video, library, and settings to 16 pixels from the left. - Location: `src/ui/Nav.svelte`, `src/ui/Watch.svelte`, `src/ui/Library.svelte`, `src/ui/Preferences.svelte` - The library utilizes the window width to display more video covers in a single row. The method for selecting playlists and expanding lists on mobile remains the same. - Location: `src/ui/Library. …

Why. In the app, display translated subtitles on videos and reopen saved videos from the library. In settings, store the private key required for using translation services and billing, and manage account connections. Previously, even when the window was maximized, the main content was constrained to a narrow width, leaving large margins, and the starting point differed for each screen. Utilize screen space while making it easier to find major tasks in settings and ensuring consistent alignment of operation buttons.

Source. PR #14 · `fix-maximized-desktop-layout`
