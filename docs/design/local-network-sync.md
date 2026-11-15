# Local-network synchronization

## Implementation status

The production goal remains automatic bidirectional synchronization of completed
translations, library entries, playlists, and playback position between Windows
and Android on a shared reachable network. Google verifies account identity;
the running Windows app is the local peer. There is no hosted sync service.

This document records the implementation boundaries. A storage component or
passing offline test is not evidence of working native synchronization. Google
registration, authentication, networking, native background operation, UI
integration, and physical-device acceptance must all be completed before the
task can be reported complete. See [Google registration](google-auth-setup.md).

`src-tauri/src/sync.rs` implements the account-scoped journal and typed transfer
format. The native account manager opens it after verified sign-in. Production
library projection is implemented as an offline storage component; account
selection in the running library, discovery and encrypted peer transfers are
not implemented yet. No sync listener opens. No existing user library has
been imported or changed.

The settings screen now starts a separate native Google sign-in: Windows uses
the system browser with a temporary loopback callback, authorization code,
PKCE S256, state and nonce; Android uses Credential Manager with a nonce and
the registered Web audience. The core verifies Google's signature, issuer,
audience, authorized party, expiry, nonce and verified email. The stable Google
subject identifies the journal; email is only a display value. Successful
Google sign-in and restoration after restart were observed on Windows and a
physical Android phone at `31ced1f`; device transfers remain unimplemented.

Windows seals the private session blob with user-scoped DPAPI. Android seals
it with its own Keystore AES-GCM key, separate from Gemini credentials. Tokens
never enter screen status or discovery. At this implementation stage an expired
ID token requires explicit sign-in; token renewal is not implemented. The UI
does not claim that identity setup enables synchronization.

The user selected explicit confirmation before importing the legacy local
library, with its original data preserved. Account registration is authorized
only without paid services or a linked billing account. The user will assist
with two real accounts and physical devices during implementation.

## Data boundary

The account's journal database also holds its materialized application tables.
Incoming records, receive cursors and durable pending projection work commit in
one SQLite transaction. The pre-account device database remains separate and
unchanged; copying its completed data requires the selected import confirmation.
Unfinished jobs are never imported. Original cues, glossaries and costs are
copied into ready local rows without transcription or Gemini requests.

Projection defers changes to a video while its local job is active. Confirmed
deletion hides saved data before managed file cleanup, retaining the existing
durable cleanup retry marker. An interrupted projection or cleanup remains
pending after restart. Different completed versions stay in the journal;
selection is deterministic and does not discard another completed response.

Each account has a separate durable sync database under an SHA-256 digest of its
Google subject and issuer. The subject must come from verified Google identity,
never a supplied email address. Directory names, discovery instance names, and
network addresses do not authorize an account. An account digest in a transfer
is an additional isolation check after authenticating the peer.

The journal stores only typed library records. It cannot execute SQL supplied
by a peer, open a supplied file path, call a translation service, or access a
credential store. Unfinished jobs, media addresses used by the downloader,
audio/video files, model files, Gemini keys, OAuth refresh tokens, and cookies
are not record types. Source page URLs are HTTP(S), contain no username or
password, and must produce the stated video key using the existing API rule.

Every transfer page is bounded in bytes and records. A receiver validates the
whole page before committing it. Unknown fields, malformed identities or
cursors, invalid URLs, incomplete translations, out-of-order cue clocks, and
invalid numeric values fail the page. A transaction rolls back if any record
fails. Cues retain the original source timestamps and recorded cost.

| Bound | Value |
| --- | --- |
| Encoded page | 16 MiB and 64 records |
| Encoded record | 8 MiB |
| Completed translation | Six hours, 100,000 nonoverlapping cues |
| Text in one source or translated cue | 16,384 UTF-8 bytes |
| Video deletion context | 64 replica high-water marks |

Oversized data is rejected, never truncated. Source language, glossary entries,
page URLs, titles, order values, and playback position have separate validation.
The glossary retains the existing `source`/`target` object shape. JSON parsing
enables exact float round trips: rounding a cost during transfer would change
the content digest and invalidate an otherwise identical retry. Translation
rates, accounting, and limits are unchanged.

## Durable merge and deletion

Each database has a random replica identifier and a durable Lamport clock.
Mutable fields merge by `(clock, replica)`; translations have separate
content-derived keys, retaining different completed versions. Playlist names,
playlist order, entry presence, entry order, and playback position are separate
records, so editing a name does not overwrite an independently changed order.
Deleting a playlist retains its translations.

A video deletion is permanent knowledge, kept as a high-water mark for each
replica that deleted that video. Data records carry the deletion marks observed
when they were written. A record missing any known mark is stale even if its
Lamport clock is higher. An offline edit cannot bypass deletion by accumulating
more edits. A newly observed deletion removes stale content from the journal;
the marks themselves remain durable. A new translation must still pass the
existing explicit retranslation confirmation and active-job safeguards.

Transfer cursors belong to the source replica, not to a shared wall clock.
Receivers persist the accepted source cursor in the same transaction as the
records. Interrupted pages do not advance it. Replayed records do not generate
new changes. A relay assigns its own sequence only to genuinely new state, so
updates can flow in both directions without an echo loop. Pagination advances
through retained current records, including durable deletion knowledge.

Receipts retain a digest for each `(replica, clock)` after content is deleted.
Reusing that stamp for different data fails the transaction. They do not retain
the deleted transcript. The session owns one journal and its WAL connection;
each operation opens a separate connection and transaction so concurrent writes
cannot share a transaction. Dropping the journal closes its WAL connection.

## Native ownership and screen integration

The core owns one account manager and shared public-key verifier. Native setup
supplies one OS credential provider and starts restoration on a blocking worker.
Each login owns its cancellation ticket; the Windows worker owns and drops its
loopback listener. The Android activity owns its Credential Manager request,
cancelling it on destruction. Sign-out or replacement invalidates old tickets
before deleting this app's saved Google session. Late completion cannot save
credentials or open a journal for the replacement account.

App exit marks all live tickets cancelled without waiting for a native callback
on the UI thread. Session operations recheck cancellation and expiry. Screen
requests capture both screen and account generations; only the action that
started a new generation may adopt its result automatically. Other changes
require an explicit status reload. Local HTTP development screens cannot start
or stop a native account; site overlays retain only P6–P8.

The signed-in native session must own discovery, the restricted listener or
client connection, cancellation, and Android background work. Listener and
discovery startup follow verified sign-in; sign-out, account replacement, and
shutdown cancel outstanding work and close owned resources. The network API
must remain separate from local Tauri commands. Remote site pages keep their
existing permissions.

Before wiring settings or library screens, capture the account session ticket
and screen generation once. Every arriving result must still match both before
it can change the view. Transfer authorization and permission to store a result
are checked separately. The database opened for an old session cannot be
reassigned to a new account.

Playback position is a durable field for the selected video. Resume may seek
once when opening a supported player. A late remote position must not seek a
player the user is already controlling. Progress received in the background
remains available for the next opening.

Android background delivery must use permitted native lifecycle mechanisms and
required notifications. Suspension can delay delivery; reopening must catch
up. A force-stopped app receives no delivery claim. Actual Windows/Android
transfers, screen-off latency, sign-out, reconnection, deletion, restart, and
two-account isolation remain required acceptance observations.

## Focused storage evidence

These checks cover the storage component and existing Gemini parsing after
enabling exact JSON float round trips. They do not exercise Google sign-in,
the production library, discovery, encrypted networking, Android lifecycle,
or physical devices.

| Command | Observed result |
| --- | --- |
| `cargo test -p video-translate-overlay --lib sync:: -- --nocapture` | 10 passed. Includes isolated account paths, restart and replay, interrupted-page rollback, concurrent writers and completed versions, relay convergence, offline deletion and explicit restoration context, bounded pagination, rejected data, and exact cost round trips. |
| `cargo test -p video-translate-overlay --lib gemini:: -- --nocapture` | 6 passed. Existing cost, glossary, translation-response, and key-error behavior remains covered. |

The test cleanup audit retained all ten sync tests and the existing Gemini
checks. Storage persistence, wire precision, concurrent writers, concurrent
versions, video deletion, and playlist deletion guard separate behaviors; no
checks were deleted or skipped. The compiler still reports the existing
`cfg(desktop)` warnings in `screens.rs`. The repository gate and final
verification remain server-owned and pending.

The projection addition passed `cargo test -p video-translate-overlay --lib
sync:: -- --nocapture`: 14 passed. After adding the remaining projection
regressions, `cargo test -p video-translate-overlay --lib sync::library:: --
--nocapture` passed all 7 projection tests. They cover actual ready-library
reads, transactional failure after cue insertion, deferred deletion, immutable
legacy copying, unfinished-job exclusion, partial dependencies across restart,
retry-queue fairness, concurrent completed versions and cleanup before fresh
restoration. The audit retained all current tests; none were removed or skipped.
These tests do not establish running account selection or network delivery.

## Focused authentication evidence

These checks cover the identity component and its local boundaries. They do not
establish LAN transfers, public OAuth configuration, real Google account
acceptance or physical Android behavior.

| Command | Observed result |
| --- | --- |
| `cargo test -p video-translate-overlay --lib account:: -- --nocapture` | 7 passed. Includes account replacement, unsent-data isolation, cancellation before native launch, late sign-in and storage-write rejection, expiry, failed-worker recovery and shutdown without a UI-thread wait. |
| `cargo test -p video-translate-overlay --lib google_ -- --nocapture` | 13 passed, including 12 signed-token/callback cases and the matching local HTTP restriction case. |
| `cargo test -p video-translate-overlay --lib commands:: -- --nocapture` | 3 passed. Local account routes are covered; Windows and Android site pages retain only P6–P8. |
| `cargo test -p video-translate-overlay --lib web:: -- --nocapture` | 5 passed, including HTTP refusal to start or end native sign-in and cross-origin rejection. |
| `cargo test -p vto-contract-tests --test settings -- --nocapture` with `VTO_CONTRACT_HOST` naming the rebuilt host | 3 passed, including actual IPC refusal of all three account commands from site pages. |
| `npx vitest run -c vitest.ui.config.ts tests/ui/google-account.test.ts tests/ui/preferences.test.ts tests/ui/updates.test.ts` | 26 passed in Chromium. Includes generations, cancellation, retired views, explicit sign-out and layouts at 384, 400 and 1280 px with production CSS. |
| `cargo build -p video-translate-overlay --features app --bin video-translate-overlay` | Passed on Windows with the existing local libclang. The isolated WebView2 screen displayed the account controls and no existing saved key. |
| `gradlew.bat -p src-tauri/gen/android :app:compileX86_64DebugKotlin -x rustBuildX86_64Debug --console=plain` | Passed with the existing local SDK and JDK 17 archive. Google ID 1.1.1 retains Kotlin 2.2 compatibility; 1.2.1 failed its metadata-version check and no compiler check was suppressed. This is Kotlin compilation, not APK acceptance. |

The test audit kept all current assertions. The native settings screen used
dirty working sources based on `840ca0c`; its embedded revision identifies that
base, not a committed acceptance candidate. Screenshots and disposable profiles
stay under ignored `artifacts/`. The full task remains incomplete.

Native preparation then rebuilt Windows at `6840c7d`. Its running core marker,
screen meta revision and actual disposable-profile digest all matched. The
settings screen reported `signed_out`, generation 1, with no email or error.
This verifies startup and the native status command, not Google authentication.
The executable SHA-256 is
`5cdef68d52691778b0eb1a5ab93fd4464a974a1e446801f2fa6301e864155e7c`.

Both Android Rust targets compiled with `cargo tauri android build --debug --apk
--target <x86_64|aarch64> --features app`. The CLI's subsequent JNI symlink step
failed because Windows disallowed symlinks. Following the README fallback,
the fresh target libraries were copied into their respective `jniLibs/` folders.
`gradlew.bat -p src-tauri/gen/android assembleX86_64Debug -x
rustBuildX86_64Debug --console=plain` and its `assembleArm64Debug -x
rustBuildArm64Debug` counterpart both completed successfully. No compiler or
signing checks were disabled.

| Debug APK | SHA-256 |
| --- | --- |
| `app-x86_64-debug.apk` | `5a0ad3f8a628e37c12f61b52b3faeeeeaee64263eee694658308cf7c7d3125780` |
| `app-arm64-debug.apk` | `eca0cdf5a158095afc5ce41ec1d55c8dc3bdf9620fb080c4acb830212b04dce9` |

`apksigner verify --print-certs` accepted both files. Both retain certificate
SHA-256 `fa6da2233c2062b48baf20b3562eacacce774da80f2e4bb3d20ec3a6a527ec28`,
matching the registered Android client's SHA-1 and the existing release key.
These are local debug builds, not approved distribution artifacts. No Android
device was connected, no APK was installed on a user's phone, and no real
Google sign-in or paid translation was performed in these preparation checks.

## Desktop registration correction

The subsequent user-assisted Windows attempt failed. The running `6840c7d`
core reported `Google identity verification unavailable`; its token exchange
mapped every provider rejection to that generic error. No Google identity was
accepted. A bounded diagnostic POST to Google's real token endpoint, using the
selected public Desktop client ID and an intentionally invalid code, returned
HTTP 400 `invalid_request` and `client_secret is missing`. No real grant or token
was sent in that diagnostic, and no response credentials were logged.

The exchange now includes the publisher's Desktop client secret through the
OIDC client library. Registration errors map to a fixed app-configuration
message; transport failures remain separate. Provider bodies are never returned
to screens. The secret never enters the authorization URL. The publisher's
Windows build reads the bounded, validated local JSON described in
[Google registration](google-auth-setup.md); Android and contract/HTTP builds
do not read or embed it. PKCE and signed identity checks remain required.

`cargo test -p video-translate-overlay --lib google_ -- --nocapture` passed 15
tests after this correction. `cargo test -p video-translate-overlay --lib
commands:: -- --nocapture` passed 3, and `npx vitest run -c vitest.ui.config.ts
tests/ui/google-account.test.ts` passed 10 in Chromium. The cleanup audit
retained current callback, signature, transport, registration and UI checks;
none were removed or skipped.

The first corrected Windows build failed its new missing-registration check.
After the user saved the Desktop JSON in the ignored project directory, both
native applications were rebuilt at `31ced1f`. A diagnostic with the configured
registration and an intentionally invalid code returned `invalid_grant`,
replacing the earlier missing-secret error. No diagnostic grant was redeemed.

## Native authentication at 31ced1f

The running Windows core and screen both identified `31ced1f4433c4506cb29f78b507b64d9687ce98d`.
An isolated app/WebView profile completed user-assisted Google sign-in and
restored `signed_in` after a complete process restart. Its executable SHA-256
was `e1238bccac958f8d85ace03eda39f7dc83009cbe697a6bb9298d816976e8d40c`.

The physical Samsung SM-G996N ran Android 15, API 35. The signed ARM64 debug APK
was installed over the existing 0.1.7 app without uninstalling. Signature
verification retained the existing certificate. Before first launch, a cold
database inspection found no active jobs and three completed translations.
The root database and sealed Gemini preference hashes stayed unchanged through
sign-in. No existing-library import, transcription or Gemini call was requested.

The installed core and screen both identified the same `31ced1f` revision.
Credential Manager sign-in succeeded and the sealed session restored after
force-stop and relaunch. A private comparison of the two journal directory
digests confirmed the same verified Google subject on both devices. Status
receipts contain state, generation and revision observations, without tokens
or email addresses. The APK SHA-256 was
`93e43938f17f9db5f9880574a40164dc0d151db1c43b8d27190ca97a0c1cf90e`.
The publisher Desktop secret was absent from all 962 APK entries.

Receipts and the pre-update APK backup remain under ignored
`artifacts/google-auth-native-20261115a/` and
`artifacts/google-auth-phone-20261115/`. These observations establish one-account
native authentication and restart, not synchronization or public availability.
Public OAuth configuration, two-account isolation, LAN transfers, Android
background delivery and the server-owned gate remain unfinished.
