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

Signed-out library requests select the original device database; Google sign-in
selects a separate account-owned local database. An explicit import confirmation
copies completed device data while preserving the original. Each screen captures
its account and screen generations and ignores a response from an older generation
or retired screen. Mutation authorization is
separate from rendering: a retired screen may finish an authorized backend
mutation, but cannot publish its result into the replacement screen. A mutation
is followed by a fresh read; failed refreshes remain visible errors.

Deletion uses an inline confirmation naming the video and warning about permanent
data removal and future translation costs. Failed cleanup hides subtitles and
keeps a retry action. Playback views recheck saved availability, discard older
subtitle responses, and clear cues after deletion. Untranslated links can be
organized and opened without creating a translation job.

Several completed responses appear in one native saved-version selector, using
the existing input styling and 44 px target. Selecting a response preserves its
cue times and cost and requires reopening the video. Initial playback may resume
once at the saved position; user input cancels delayed resume and later remote
position changes never move a player already controlled here.

People use the Google account section in `/preferences` to choose the identity
for the local-network connection, separately from YouTube and Gemini.
It uses the same approved palette, card spacing, type hierarchy and 44 px
controls. Show the verified email, progress, cancellation and recovery inline;
confirm sign-out and preserve existing saved data. A signed-in identity alone
must not appear as an established sync connection. Account responses must match
the captured account generation as well as the screen generation.

Public app-information pages have two reading tasks: understand the app and
Google identity use on the homepage, and read the complete data practices on
the privacy page. They reuse C1 and the 14/16/20/28 px type scale. A 70ch reading
column, wrapping navigation, 44 px navigation targets and normal document flow
serve both phone and desktop. They have no forms, scripts or account dashboard.
