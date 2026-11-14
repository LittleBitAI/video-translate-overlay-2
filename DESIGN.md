---
colors:
  primary: '#4f8cff'
  surface: '#111418'
  text: '#f2f4f7'
  accent: '#ffd24a'
typography:
  family: system-ui
  sizes: [14, 16, 20, 28]
spacing: [4, 8, 16, 24]
rounded: [8]
components: [address-bar, library-list, playlist-list, confirmation]
---

# Local video library

People return here to reopen a translated lecture or manually choose a video
from a topic playlist. Success is the selected video opening in the existing
browser with its saved subtitles. The library at `/library` has a title/link
search, saved availability, and ordered topic playlists. It does not need a
dashboard or another video detail screen.

Use the existing approved charcoal theater palette C1. Blue identifies actions;
yellow identifies errors and permanent deletion warnings. The existing system
font uses 28 px for the page heading, 20 px for playlist headings, 16 px for
controls and titles, and 14 px for source links and availability. Controls have
at least 44 px touch targets. Rows and controls wrap at phone widths; links wrap
without hiding their destination.

All requests belong to this device's SQLite database, with no accounts or cloud
scope. Each screen captures its generation when a request begins and ignores a
response from an older generation or retired screen. Mutation authorization is
separate from rendering: a retired screen may finish an authorized backend
mutation, but cannot publish its result into the replacement screen. A mutation
is followed by a fresh read; failed refreshes remain visible errors.

Deletion uses an inline confirmation naming the video and warning about permanent
data removal and future translation costs. Failed cleanup hides subtitles and
keeps a retry action. Playback views recheck saved availability, discard older
subtitle responses, and clear cues after deletion. Untranslated links can be
organized and opened without creating a translation job.
