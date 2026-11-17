---
scope: project
severity: preference
triggers: []
domain: ''
title: "Provides guidance on approved new versions and device-specific downloads for Android and Windows."
pr: 5
merged: 2026-11-14
branch: "approved-release-update-notices"
---

# Provides guidance on approved new versions and device-specific downloads for Android and Windows.

What. When opening the app or returning from another screen, it displays the approved new version number, a brief feature description, and a download link. It also displays the app's guidance window even when a video site is open. - Location: `src/ui/Updates.svelte`, `src-tauri/src/app.rs`, `src-tauri/gen/android/app/src/main/java/dev/vto/overlay/VtoPlugin. …

Why. Added new version guidance to the app that attaches and saves translated subtitles to videos. Users read the added features and download the installation file suitable for their device from the browser to install it themselves. This reduced the inconvenience of users having to separately look for update availability and installation files. It prevents the risk of guiding to unapproved builds or incorrect download addresses, and ensures that video usage is not interrupted even if there is a problem with the verification server.

Source. PR #5 · `approved-release-update-notices`
