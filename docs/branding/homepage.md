# Video Translate Overlay

Draft for owner review. This text is not a live website. Replace
`PUBLIC_SUPPORT_EMAIL`, `HOME_PAGE_URL`, and `PRIVACY_POLICY_URL` with approved
values before publication; remove this draft notice from the published page.

Video Translate Overlay is an installed Windows and Android app for watching
foreign-language lectures with Korean subtitles. It downloads supported audio,
transcribes speech on the device, translates the resulting text with Gemini,
and saves subtitles on the original speech timeline. Translation uses the
user's own Gemini API key and may incur API charges.

## Google sign-in and nearby-device synchronization

Google sign-in identifies the account used for the app's local library. It
uses basic identity information, including a verified email address and stable
Google account identifier. App sign-in is separate from signing in to YouTube.
It does not request access to Google Drive, Gmail or YouTube account data.

In Google sign-in enabled builds, a running Windows app and an Android app
signed in to the same account can exchange completed subtitles, video links,
playlists and playback positions over a reachable local network. Ethernet on
the PC and Wi-Fi on the phone can share that network. Transfers use encrypted,
authenticated connections. Library data is stored on each device, with no
publisher-operated cloud synchronization database or relay.

Unfinished processing jobs, original media files, speech models, Gemini API
keys and YouTube cookies are not synchronized. Confirmed video deletion also
propagates to the same account's connected devices and catches up after a
disconnected device reconnects. Android background delivery can be delayed by
the operating system; reopening the app catches up.

## Availability

The approved 0.1.7 release keeps separate device-local libraries. Google app
sign-in and local-network synchronization are development features whose native
Windows and Android builds have been exercised; they are not included in that
published release. This page does not announce a new APK or installer.

- [User guide and source code](https://github.com/LittleBitAI/video-translate-overlay-2)
- [Approved downloads](https://github.com/LittleBitAI/video-translate-overlay-2/releases)
- [Privacy policy](PRIVACY_POLICY_URL)

Support: `PUBLIC_SUPPORT_EMAIL`.
