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
components: [shared-menu, address-bar, playlist-rail, playlist-accordion, video-card, edit-mode, add-panel, confirmation]
---

# Local video library

People return here to reopen a translated lecture or to go through a topic
playlist. Success is the chosen video opening on the watch screen with its
saved subtitles. Activating a playlist shows its videos at once, with no
scrolling and no second screen. The library at `/library` has a title/link
search, saved availability, and ordered topic playlists. It does not need a
dashboard, a playlist page or another video detail screen.

From a 768 px window width (the Windows window, a tablet or a phone held
sideways) a 16 rem left rail lists 전체 영상 and every playlist with a cover,
its name and its video count. The selected one has a blue left edge and tint
and `aria-current`. The right pane shows its videos as a grid of cards no
narrower than 13 rem. The rail stays in place while the grid scrolls, and the
rail header is as tall as the pane's toolbar so the first playlist and the
first card share a top edge. Below 768 px the screen is one column: tapping a
playlist row expands its videos directly under it (`aria-expanded`), tapping
again closes it, and one row is open at a time. The open row stays pinned to
the top while its videos scroll. A search with no row open shows 전체 영상.

A video card is a 16:9 thumbnail with the duration in its corner when known,
the title clamped to two lines, the site name and the saved, processing or
cleanup-pending status. A YouTube thumbnail comes from YouTube's image server by
the stored video ID. Other sites, and any image that fails to load, get a tile
with the site host and a play mark. Cards show the site name, not the address:
the full address appears in edit mode. Activating a card opens the video
through the same request as before. A playlist's cover is its first video's
thumbnail, or the tile when it is empty. Wide cards stack the thumbnail over the
text; phone cards put a 144 px thumbnail beside it.

The default view has only the search, 추가 and 편집. 편집 (then 완료) lists the
videos in one column, thumbnail beside title, and reveals a drag handle and visibly labeled 위로 / 아래로 on playlists and entries, the
rename field, 목록에서 빼기, 영상 삭제 and 재생목록 삭제. Dropping a handle sends
one move of however many places it crossed; the buttons move one place and are
the keyboard path. 추가 opens a panel with a saved-video picker showing
thumbnails, a link field with an optional name and the cost note, and
+ 새 재생목록, which selects the playlist it creates. A refresh after a change
keeps the shown library and scroll position until the new answer arrives.

영상, 보관함 and 설정 share one menu in the same place on every screen: tabs at
the left of the top bar from 768 px, a bottom tab bar with 44 px targets and the
safe-area inset below that. The current one has `aria-current="page"`, weight
and a blue bar. The address field stays only on `/`.

Use the existing approved charcoal theater palette C1. Blue identifies actions
and the selected place; yellow identifies errors, processing and permanent
deletion. The existing system font uses 28 px for the page heading, 20 px for
the pane and panel headings, 16 px for controls, playlist names and (semibold)
card titles, and 14 px for site names, counts, status and durations. Controls
have at least 44 px touch targets.

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

Several completed responses are counted on the card's status line and appear in
one native saved-version selector in edit mode, using the existing input
styling and 44 px target. Selecting a response preserves its
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

Keep a 44 px privacy-policy control in that same account section, available
before sign-in. Its native action opens only the published policy in the
platform browser and shows a retryable error without changing account state.

Public app-information pages have two reading tasks: understand the app and
Google identity use on the homepage, and read the complete data practices on
the privacy page. They reuse C1 and the 14/16/20/28 px type scale. A 70ch reading
column, wrapping navigation, 44 px navigation targets and normal document flow
serve both phone and desktop. They have no forms, scripts or account dashboard.

## Overview site

`overview.html`, `overview/guide.html` and `overview/install.html` are the
Korean introduction. Its reader has found the repository and wants to decide
whether to install. Success means one of the two download buttons in the first
screen, or the install page.

- Order of the intro page: claim and downloads with the Windows library capture,
  the four steps behind 영상감지, three measured numbers with their source, the
  library (text beside the Android library capture), PC and phone sharing
  (conditions as a check list, then the Windows and Android player captures of
  the same synced subtitle), what does not work yet as plain sentences, and the
  two download buttons again. Each heading states a user outcome, not a
  development story. No build log: version history lives in GitHub Releases.
- Copy must agree with its capture. When a capture shows `자막 없음`, the text
  explains that state instead of claiming saved subtitles. Body text does not
  narrate what the capture already shows, and a caption does not repeat the
  body. A number appears once in the stats and is not restated below.
- Voice is 합니다체, short sentences, app terms exactly as on screen. App
  button and menu names use the `.ui` chip (sans, 600), never `code`, because
  they are screen words, not code.
- Colour stays the app's blue, chosen by the user on 2026-11-17 over a subtitle
  amber and an ink-only option: `#2c66cc` on the light `#f5f4f1` ground (4.9:1)
  and `#4f8cff` on the dark `#14161a` ground (5.6:1). It marks big numbers,
  step numbers and links, so the page reads as the same product as the captures.
- Type levels: serif display h1, serif h2, serif stat number, sans lead,
  sans body, sans item title (17px, 650), sans caption, mono date and label.
  One level carries one meaning.
- Layout: one four-column grid (`--cols`, one `--gap`) for all three pages.
  Every block starts on one of its four lines; no section invents its own
  split. Reading text spans two columns (`--half`), a portrait phone capture
  takes one column (the third beside text), a landscape capture takes two or
  three. The four steps take one column each. Two sections are the exception,
  chosen by the user on 2026-11-17: the three numbers fill the page in three
  equal columns with the measurement source as one line below them, and the
  library section below uses the same three columns, text across the first
  two and the 16rem phone capture starting on the third (the 41분 line). Below
  960px every grid becomes one column; below 600px the download buttons go
  full width. Check in a browser: at 1440 and 1100px every grid child's left
  edge is on a column line of its grid, and nothing overflows at 1440 or 400px.
- Only real captures are used, uncropped and at their own aspect ratio. A
  portrait phone screen is never cut into a landscape strip. Nothing is drawn
  to look like the app.
