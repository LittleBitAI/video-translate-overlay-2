# Video Translate Overlay privacy policy

Draft for owner review, prepared 2026-11-15. Do not publish until
`PUBLIC_SUPPORT_EMAIL`, `HOME_PAGE_URL`, and `HOSTING_PRIVACY_NOTICE` have been
completed and approved. Remove this draft notice from the published page.

This notice describes Video Translate Overlay on Windows and Android. Sections
about Google app sign-in and local-network synchronization apply to builds
containing those features. The approved 0.1.7 release does not contain them.

## Google account information

When you choose Google app sign-in, Google supplies basic identity information:
your verified email address, a stable account identifier, and a signed identity
token. The app uses this information to display the selected account, separate
its local library from other accounts, and verify device identity for nearby
synchronization. It requests identity scopes only. It does not request Google
Drive, Gmail or YouTube data through this app sign-in.

Google handles the sign-in screen. The app does not receive your Google
password. Its native sign-in session and device private key are sealed with
Windows user-scoped DPAPI or Android Keystore protection. Local device
enrollment lasts at most thirty days, after which transfers require renewed
sign-in. New sign-ins do not request or retain OAuth refresh tokens.

During encrypted device authentication, the app can exchange a Google-signed
identity proof bound to its device certificate. Matching verified account
identities are required before library transfer. Anonymous discovery messages
do not contain your account identifier, email address or credentials. The
publisher does not operate a server that collects your sign-in tokens or
stores your synchronized library.

The app does not sell Google sign-in information, use it for advertising, or
send it to speech-recognition or translation models. Google processes sign-in
requests under its own [Privacy Policy](https://policies.google.com/privacy).

## Local library and device transfers

The app stores video titles and source URLs, transcripts, translated timed
cues, glossaries, recorded translation costs, playlists and playback positions
in local SQLite files. Those library files are separate from the operating
system's protection of credentials; this notice does not claim that the
SQLite files themselves are encrypted.

When both devices use the same Google app account on a reachable local
network, completed library records synchronize through authenticated encrypted
connections. Copies remain on the receiving device for later use. Original
media playback still depends on its source and network access. No hosted
library database, Google Drive storage or cloud relay is used for this feature.

Synchronization excludes unfinished jobs, original video and audio files,
speech models, Gemini API keys, Google OAuth refresh/access tokens, and YouTube
browser cookies. Existing unowned device-library data is copied to an account
only after an explicit import confirmation. Account switching selects a
different local account library and stops the old account's transfers.

## Transcription, translation and other network requests

Speech recognition runs on the device. When you explicitly request
translation, the app sends transcript text, translation context and glossary
content to Google's Gemini API using your own API key. Google processes those
requests under the terms and data practices applicable to your Gemini account.
Signing in to the app or opening a saved translation does not itself request
a new transcription or Gemini translation.

Gemini API keys use Windows Credential Manager or Android Keystore-backed
storage. They are not part of the synchronized library. YouTube sign-in uses
the app browser's separate session, whose cookies stay in its local WebView
profile. The app also connects to media sources, speech-model download sources
and the approved GitHub Releases channel when those features are used. Those
services receive the network requests needed for the selected operation.

## Retention and your controls

Signing out stops that app's account transfers and removes its saved sign-in
credential. Previously synchronized library copies remain local to their
accounts. Signing out does not erase other devices. Changes to your Google
account or permissions elsewhere do not immediately invalidate the app's
existing thirty-day local device enrollment; sign out in the app on the
relevant devices to stop their transfers.

Confirmed video deletion removes the video's stored transcript, subtitle
responses, glossary, job history, managed per-video files and playlist
memberships. Connected devices apply the deletion; disconnected devices apply
it after reconnecting. Durable deletion markers and synchronization receipts
remain to prevent stale copies from resurrecting deleted records. File-cleanup
failures remain visible for retry. Deleting a playlist or removing one entry
keeps the underlying translation.

Deleting a Gemini key or signing out of YouTube uses separate settings
controls. Removing local application data removes that device's copy; it does
not constitute a deletion request to other disconnected devices. Windows
uninstallation may retain local application data. There is no publisher cloud
account database requiring a separate remote-erasure request.

## Website visits and support

`HOSTING_PRIVACY_NOTICE`

If you contact support, the publisher receives the address and information you
choose to send. Do not send passwords, API keys, sign-in tokens or browser
cookies in support requests or public issue reports.

Support and privacy questions: `PUBLIC_SUPPORT_EMAIL`.

[Application homepage](HOME_PAGE_URL)
