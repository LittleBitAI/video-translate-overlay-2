---
scope: project
severity: contract
triggers: ["화면", "vision", "프레임", "스크린", "캡처", "공유"]
domain: vision
title: "When you tap a playlist in the library, the videos appear immediately as thumbnails, and all three screens use the same menu."
pr: 9
merged: 2026-11-17
branch: "library-rail-accordion-shared-menu"
---

# When you tap a playlist in the library, the videos appear immediately as thumbnails, and all three screens use the same menu.

What. If the window width is 768 pixels or more, the left vertical list displays 'All Videos' and all playlists along with their cover images, names, and video counts. The selected playlist is highlighted, and its videos appear immediately as a thumbnail grid on the right. On narrow screens like mobile phones, the videos expand directly below the tapped playlist. Tapping again collapses it, only one can be open at a time, and the open row remains pinned to the top of the screen when scrolling. - Location: `src/ui/Library.svelte`, `tests/ui/library-layout. …

Why. The library is a screen for reopening videos with saved translated subtitles and organizing them into topic-based playlists. Until now, even when a playlist was tapped, it was pushed down by the management form, placing the first video about 1,200 pixels below the screen. As a result, it looked like nothing happened when tapped. Because the videos were off-screen when a playlist was tapped, users felt like nothing had occurred. Buttons with only arrows wrapped to the next line on mobile phones, making it difficult to tell which item they belonged to.

Source. PR #9 · `library-rail-accordion-shared-menu`
