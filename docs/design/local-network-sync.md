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
format. It is not wired into the current app and opens no listener. Its focused
tests are in `src-tauri/src/sync_tests.rs`. No existing user library is migrated
or changed by this component.

The user selected explicit confirmation before importing the legacy local
library, with its original data preserved. Account registration is authorized
only without paid services or a linked billing account. The user will assist
with two real accounts and physical devices during implementation.

## Data boundary

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
