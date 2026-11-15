# Local-network synchronization

## Implementation status

The user selected a thirty-day native device sign-in. A fresh Google proof is
required to enroll an unknown device certificate; subsequent connections may
use a locally stored trust receipt for that certificate until its fixed expiry.
The device's private key remains sealed by DPAPI or Android Keystore. App
sign-out stops its connections immediately; changing a Google password elsewhere
does not immediately revoke this local credential. New or expired credentials
require fresh Google verification. This policy needs no hosted service or paid
call. It supersedes the temporary one-hour proof cutoff described in historical
implementation notes below. Both native apps restored this enrollment without
interactive sign-in after process restart at `fc0d072`. Expired-proof reconnection
and the fixed thirty-day expiry are covered offline; no thirty-day elapsed-time
device run is claimed.

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
library projection and account selection are connected to the library APIs.
Discovery and encrypted peer transfers are now connected to a native controller.
Only native app setup starts it; development HTTP and contract hosts do not open
the sync listener. One-account native flows passed at `fc0d072`; screen-off
Ethernet/Wi-Fi delivery and two-account isolation passed at `5f899a2`. The owner
confirmed Google's External audience is `In production` after the approved
public branding configuration. The public homepage and privacy policy are live,
and native settings privacy access passed on both platforms at `ff30ca8`; see
[registration and final native evidence](google-auth-setup.md#native-privacy-access-at-ff30ca8).
The physical phone's three original completed translations and sealed Gemini
credential remained unchanged after the final APK update. No existing user
library has been imported. The server-owned repository gate and final
verification remain pending.

The settings screen now starts a separate native Google sign-in: Windows uses
the system browser with a temporary loopback callback, authorization code,
PKCE S256, state and nonce; Android uses Credential Manager with a nonce and
the registered Web audience. The core verifies Google's signature, issuer,
audience, authorized party, expiry, nonce and verified email. The stable Google
subject identifies the journal; email is only a display value. Successful
Google sign-in and restoration after restart were observed on Windows and a
physical Android phone at `31ced1f`; that evidence predates the native controller.

Windows seals the private session blob with user-scoped DPAPI. Android seals
it with its own Keystore AES-GCM key, separate from Gemini credentials. Tokens
never enter screen status or discovery. A native enrollment receipt retains the
account's library ownership for thirty days, bounded by the certificate's actual
expiry. Expiry stops transfers but retains access to that account's local data.
OAuth refresh tokens are not requested or retained by new sign-ins. The UI does
not claim that identity setup enables synchronization.

New sign-ins generate a per-login ECDSA device key and anonymous certificate.
The Google nonce is the certificate's SHA-256 digest. The key is sealed with
the native session and checked against that nonce on restoration; replacing an
account creates a new key. Earlier sessions without a key can still restore
identity status, but cannot authorize a peer connection and require a fresh
sign-in before LAN enrollment. The native evidence at `31ced1f` predates this
binding and does not validate it.

`sync_tls.rs` prepares mutual TLS 1.3 with a dedicated `vto-sync/1` ALPN.
WebPKI checks the bounded certificates, dates, names and key usages; ring checks
the actual TLS handshake signatures. Possession of a self-issued key does not
authorize a library request. After the handshake, the peer's Google-signed
proof must contain that exact certificate digest as its nonce and the same
verified subject as the local session when enrolling a new certificate. Later
connections may use the account journal's private certificate trust receipt;
neither replay nor reconnection extends its fixed expiry. A copied proof with
another private key fails. TLS resumption, session tickets and early data are disabled. The module
opens no sockets and performs no discovery; the owning session releases its
private key when retired sessions are dropped. The native controller owns sockets
and invokes this transport. Native delivery observations and their limits are
recorded below with their source revisions.

`sync_channel.rs` owns an already-connected TCP socket and requires mutual TLS
before its bounded proof exchange. The Android client verifies the PC's bound
Google proof or previously enrolled certificate before sending its own proof.
Only matching signed Google subjects authorize enrollment. An unknown certificate
with an expired Google proof requires fresh sign-in, without clearing locally
saved data. The dedicated authentication phase carries a certificate-bound ID
proof when needed; it never sends OAuth access/refresh tokens, the publisher
registration secret, Gemini keys or cookies. Library pages remain behind this
authorization gate and have no general command, SQL or file-access operation.

Control frames are capped at 16 KiB, and library frames contain one bounded
page plus framing overhead. Handshakes have a five-second deadline and each
frame has a thirty-second absolute deadline. Socket I/O checks cancellation
in slices of at most 250 ms. Each connection worker owns its socket and session;
dropping the channel closes its socket, without a manager/socket ownership cycle.
Account cancellation prevents further page reads, writes and journal commits.
The worker must discard a channel after any failed exchange. The native controller
below owns discovery, listeners and the Android lifecycle lease.

`sync_runtime.rs` creates one controller in native setup and hands only a weak
core reference to its owner thread. Each verified account/network lease owns its
DNS-SD daemon, nonblocking Windows listeners and cancellation flag. Android
discovers Windows advertisements and initiates connections; each authenticated
connection exchanges one page in both directions. Successful peers are checked
again after two seconds, without an address entry or manual sync control. Failed
attempts back off from two to thirty-two seconds. Interface changes replace the
lease; old account or network outcomes cannot update its replacement.

There are at most four pending or connected workers, sixteen discovered peers,
and sixty-four recent incoming source addresses. Incoming sources are throttled
per address. An outbound socket is bound to its selected local interface address
before connecting, and its actual local and peer addresses are checked. Public,
loopback, multicast and out-of-prefix targets fail before a connection opens.
Each worker owns its socket; the controller holds only a shutdown clone while
it runs. Retiring a lease shuts down those sockets, and worker guards release
their slots. Completed worker threads are joined. On app exit, the UI requests
stop and a background shutdown thread waits for the controller, discovery
acknowledgement and remaining workers before allowing exit. Dropping the native
controller also signals stop and shuts down registered sockets.

Native status contains a connection state, account generation and optional last
success time, without peer addresses, device names or account identifiers.
Discovery, listener, authentication, account mismatch, interrupted transfer and
background-service failures have distinct recovery text. The search message
groups PC unavailability, blocked discovery, firewall and network isolation;
it does not claim to diagnose which one occurred. A recent authenticated page
exchange is required before `sync_ready` is true. A peer needing a fresh Google
proof can request explicit sign-in renewal without erasing the local library.

Android uses a separate non-exported `SyncService`, with an ongoing notification
and a multicast lock. Its native controller registers a random ownership token;
retired controllers and account generations cannot restart or stop a replacement
service. Service creation requires a visible activity and enabled notifications.
If those conditions fail, the Rust controller closes its lease and reports
background pause. Returning to the activity retries the requested service and
automatic discovery. Sign-out, credential expiry and controller shutdown release
the service and multicast lock. It uses `START_NOT_STICKY`; a killed process
does not claim delivery until the app is reopened.

The `connectedDevice` type and `FOREGROUND_SERVICE_CONNECTED_DEVICE` permission
cover network interaction with an external device; the normal
`CHANGE_WIFI_MULTICAST_STATE` permission supplies its documented prerequisite.
See [Android foreground service types](https://developer.android.com/develop/background-work/services/fgs/service-types#connected-device).
This service holds no permanent CPU wake lock. Android suspension may delay
delivery. Measured screen-off latency and its environment limits are recorded
below; no deep device-idle guarantee is made.
The translation job's existing service and wake-lock policy are unchanged.

Automatic cleanup of remote video-deletion markers is connected to the native
controller. Playback persistence and one-time resume are connected to the native
overlay. The controller's existence and offline protocol tests alone are not
evidence that those flows work on devices.

Playback uses typed get/set modes of existing P7, without additional site
permissions. Writes accept only integer positions within a bounded player
duration, for an existing non-deleted video in the captured account. Saved
translations additionally require the player duration to match the original
timeline within one second. Position and journal writes commit atomically;
they do not modify cues, costs or processing jobs. Deletion removes the local
position, and cleanup reapplies any surviving explicitly restored position.

Each mounted native player reads its resume position once. It waits through
advertisements and missing metadata, then attempts one seek. Existing playback
past the opening second, user input or a user seek cancels a delayed resume;
later remote updates never move that mounted player. Actual playback writes
are coalesced, at most once per five seconds with pause/seek/hidden flushes.
Account retirement detaches that owner's updates. Unsupported seeking leaves
playback under the player's normal controls. No play, pause or speed command
is issued by synchronization.

`sync_discovery.rs` selects operational private IPv4, IPv6 ULA and scoped
link-local interfaces. Point-to-point and public addresses are excluded.
Listeners bind the selected addresses rather than a wildcard, using one
ephemeral port; accepted sources must be private unicast peers on that local
prefix. Discovery candidates undergo the same address checks before use. At
most 16 local addresses and 32 candidate addresses per service are accepted;
oversized sets fail rather than silently selecting an arbitrary subset.

The DNS-SD service is `_vto-sync._tcp.local.`. Its instance and hostname are
random per advertisement, and its only TXT field is `v=1`. No Google subject,
email, account digest, device key, token or computer name enters the
advertisement factory. Only explicit addresses are published; automatic address
expansion is disabled. The discovery lease owns the daemon, error monitor and
subscription. It unregisters, stops browsing and requests acknowledged shutdown
when released, with a bounded retry for a full command queue. An unacknowledged
shutdown produces a fixed diagnostic without untrusted network values.

These components are not yet started by the native applications. Their owner
must still connect sign-in, network replacement, worker cancellation, connection
limits and Android background lifecycle before enabling production discovery.

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
When several saved responses exist, the library exposes their retained IDs in
a native select control. Selecting one republishes that same completed content
with a newer journal stamp and projects its original cues and cost atomically.
This selection converges through the existing transfer format; no new transcript
or paid request is made. Selection refuses active/deleted videos and cannot
address another account's record. Reopen the video to load the selected cues.
Job reads use the selected completed response even when two ready-job timestamps
tie; active or failed-job status keeps its existing meaning.

Retranslation uses the highest recorded cumulative cost across the video's job
history and retained completed responses, including versions never projected on
this device. Selection cannot roll that baseline back. The store rechecks it in
the job-creation transaction; later provider usage adds only the new call's cost
to the retained amount. A remote saved response need not have its original token
counters to preserve that amount. Reading versions remains bounded to one
response at a time. The model, unit prices, exchange rate and duration-based
limit are unchanged.

The existing Store mutation paths append typed records inside their originating
SQLite transactions when the database belongs to an account. Legacy stores do
not acquire sync tables or publish records. Incoming projection writes tables
directly, avoiding an echo through those local mutation paths. Concurrent equal
order values use stable playlist IDs and video keys as visible tie breakers.

A native job captures deletion knowledge when created, including retranslation
jobs. Completion publishes the complete response with that captured knowledge;
it cannot adopt a deletion received during processing. Both normal job creation
and retranslation recheck deletion inside their write transaction. Completed
cleanup remembers which deletion marks it covered, so another deletion arriving
before projection invalidates an older confirmation token. Cleanup retries do
not create additional deletion intents.

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

Settings and library screens capture the account session ticket
and screen generation once. Every arriving result must still match both before
it can change the view. Transfer authorization and permission to store a result
are checked separately. The database opened for an old session cannot be
reassigned to a new account.

The request's library scope now holds its original account session and SQLite
connection. Short database operations check its generation and session pointer
under the manager lock; native navigation, notifications, media probing and file
cleanup run outside that lock. A media check cannot create a job after its
account scope retires. Native requests carry the generation shown by the screen,
and account events discard cached cues, pending confirmations and late replies.
A newly injected overlay can bootstrap only the public generation counter from
P7, without gaining account-status permissions or receiving identity data.

Signed-out requests read the existing root database. Signed-in requests read
their account database, including after the device credential expires; expiry
only disables networking. The library offers an explicit confirmed copy of the
unowned completed library and playlists, preserving the source. No other account
database is an import source. Its content receipts prevent repeat imports from
resurrecting deleted items. The offer compares current content receipts, so new
unowned items added after an earlier copy can be offered again. Saved playback
positions accompany copied video entries. A malformed original library leaves
the account library readable and makes confirmed import fail without changing
its source. This offer is implementation work, not evidence that
any installed user's data has been imported.

The single translation worker inspects only fixed app-owned database names and
continues a captured job in its owning database after an account change. Global
job admission and the native close guard check all these databases. Native
notification output suppresses a retired account's title and screen URL; old
notification activations resolve only against the currently selected library.
The worker's operation-local connections close after each pass, while its
captured job connection lasts through processing and recovery from a panic.

The native controller also owns a separate local-maintenance worker. Every two
seconds it projects the selected account's pending records and scans up to
sixteen incomplete deletion markers, including markers without visible video
rows. It captures the account's local library scope, so expiry can finish an
already-authorized local cleanup while network access remains stopped. Sign-out
retires that scope, and another account is never substituted mid-operation.

Cleanup shares job admission's lock, skips active videos, and removes only files
whose basenames begin with an owned job ID in the managed audio directory.
Original media, models, credentials and unrelated jobs are outside that prefix
selection. It does not follow a stored audio path. Missing files or an absent
audio directory are already clean. Failures keep the marker for durable retry;
a rotating key cursor prevents one failing file from starving other markers.
Finalization and the next projection run through short guarded database
operations. Network listeners and discovery can close independently of filesystem
work. The controller joins this maintenance worker during shutdown.

The signed-out unowned library keeps its explicit retry behavior. Maintenance
does not import it or process its failed deletions automatically. Native ready
notification activations also reject an incomplete deletion marker. Library and
watch views refresh after applied remote changes; an in-flight open keeps its
operation until the queued refresh can read its resulting browser pointer.

Playback position is a durable field for the selected video. Resume may seek
once when opening a supported player. A late remote position must not seek a
player the user is already controlling. Progress received in the background
remains available for the next opening.

Android background delivery must use permitted native lifecycle mechanisms and
required notifications. Suspension can delay delivery; reopening must catch
up. A force-stopped app receives no delivery claim. Actual Windows/Android
transfers, screen-off latency, sign-out, reconnection, deletion, restart, and
two-account isolation are required acceptance observations; the native evidence
below records the exercised builds and environments.

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

Local mutation integration then passed all 21 sync tests and all 5 existing
Store tests. The final deletion-race regression expanded the projection suite:
`cargo test -p video-translate-overlay --lib sync::library:: -- --nocapture`
passed 12. After extending the existing completion test to cover retranslation,
`cargo test -p video-translate-overlay --lib sync::library::tests::native_completion
-- --nocapture` passed 1. An injected journal failure rolls back the native ready
mark; incomplete replacement text never publishes, and both completed versions
survive a later replacement. No tests were deleted or skipped.

`cargo test -p video-translate-overlay --lib
pipeline::worker_tests::retranslation_resumes_partial_work_and_publishes_only_a_complete_set
-- --nocapture` passed 1. After rebuilding `vto-contract-host`, `cargo test -p
vto-contract-tests --test library -- --nocapture` passed all 5 library contracts,
including reuse without paid calls, active-job refusal and cleanup retry. These
are focused checks; the server-owned repository gate has not run.

The TLS preparation passed `cargo test -p video-translate-overlay --lib
sync_tls:: -- --nocapture`: 4 passed. Real TLS handshakes in memory check both
keys, matching and mismatched Google subjects, audience and expiry, a copied
proof presented with a different key, replayed ciphertext on a fresh channel,
missing client certificates, wrong ALPN, malformed/oversized certificates,
mismatched private keys and expired certificates. Key serialization retains
the nonce binding. These use the committed test-only RSA signing fixture,
not Google-issued credentials or a native network connection.

`cargo test -p video-translate-overlay --lib account:: -- --nocapture` passed
8 after the per-login key change. `cargo test -p video-translate-overlay --lib
google_ -- --nocapture` passed 19, including the matching TLS/account test
names. The audit retained every current security and lifecycle check. No
native build or real Google sign-in has yet exercised the certificate nonce.

`cargo test -p video-translate-overlay --lib sync_channel:: -- --nocapture`
passed 5 tests using owned loopback TCP sockets and test-signed Google proofs.
They cover completed translations and phone-created playlist changes in both
directions, usable SQLite data after reopening, convergence without echo,
mismatched subjects despite identical email, sign-out during a blocked read,
bounded/strict framing, and reconnection after only one direction committed.
These are local protocol tests, not real Google-account or Windows/Android
device-transfer acceptance. Duplicate RSA fixture-building code was folded
into the existing authentication fixtures; all behavior checks remain.

`cargo test -p video-translate-overlay --lib sync_discovery:: -- --nocapture`
passed 6 after the final dependency configuration, with optional async and
discovery logging features disabled. The cases cover selected-interface
filtering, Ethernet/Wi-Fi peers on the same prefix, public and broadcast
refusal, private IPv6 scope checks, anonymous advertisements, bounded candidates,
owned listener release and a real loopback DNS-SD announcement followed by an
acknowledged daemon shutdown. The DNS-SD check uses a disposable UDP port and
only the loopback interface; it does not establish discovery between devices.
All six tests remain. The compiler also reports currently unused native-worker
entry points while application integration is unfinished.

A subsequent projection regression reproduced a deletion and fresh link
arriving together before local cleanup: the new metadata could keep the old
ready response visible. Projection and job admission now require cleanup to
cover every known deletion mark, even when the same incoming page also contains
a fresh link or an explicitly restored translation. `cargo test -p
video-translate-overlay --lib sync::library:: -- --nocapture` then passed all
13 projection tests. The new table checks both link-only and translated
restoration; existing completed-version, active-job and consent checks remain.

## Focused authentication evidence

The cleanup addition passed `cargo test -p video-translate-overlay --lib
sync_runtime:: -- --nocapture`: 7 tests, including the existing five controller
cases and two filesystem/database scenarios. They cover an unrelated active
job, exact file-prefix selection, preserved models and original library, orphan
markers, replay after cleanup, active-video deferral, a failed directory removal,
twenty other deletion markers, sign-out and automatic retry after returning to
the account. The Windows app `cargo check` passed after this addition.

The focused watch/library Chromium run passed 25 tests, including a queued
remote refresh during opening. Vitest reported that its browser did not close
within thirty seconds and would be killed on runner exit; test exit was zero.
This observation is not a clean browser-shutdown receipt. No checks were removed
or skipped in the cleanup audit; existing deletion confirmation/retry checks
still exercise the shared file-removal helper.
After rebuilding `vto-contract-host` with the final shared cleanup helper,
`cargo test -p vto-contract-tests --test library -- --nocapture` passed all 5
contracts, including active-job refusal and explicit cleanup failure/retry.

The native-controller addition passed `cargo test -p video-translate-overlay
--lib sync_ -- --nocapture`: 22 tests across discovery, TLS, protocol and runtime
ownership. The five runtime cases cover bounded pending workers, shutdown of an
actual socket, refusal to attach after lease retirement, bounded retry timing,
signed-out controller shutdown and refused Android background ownership (some
checks share a case). The existing discovery table now also checks the outbound
connection refusal boundary. These tests use loopback or refused interfaces;
they do not start an authenticated listener on the user's LAN.

The Windows app `cargo check` passed with `--features app`. Android
`:app:compileArm64DebugKotlin -x rustBuildArm64Debug --console=plain` passed with
the existing JDK 17 and SDK, including the final service timeout hook. These are
compilation checks, not installed-app acceptance. The focused Google account and
account-library Chromium files passed 16 tests, including automatic recovery
status, explicit new-peer reauthentication and retirement of a displayed account.
The expiry-copy assertion now checks retained local access as well as sign-in
recovery. The audit retained every affected check and added no skips or expected
failures. Native build, two-account and background evidence remain pending.

Account-library integration passed all 14 `account::` tests, including production
API isolation, named import consent, retained original cue intervals and cost,
retired confirmations and a real pending media check during account replacement.
`cargo test -p video-translate-overlay --lib pipeline::worker_tests::a_job --
--nocapture` passed 2: root and unselected-account jobs resume, complete and keep
their owning data. `commands::` passed 3 existing ACL and route checks. These
fixtures exercise no real Google authentication, transcription or paid call.

The focused Chromium screen run of `tests/ui/library.test.ts`,
`tests/ui/watch.test.ts`, `tests/ui/google-account.test.ts` and
`tests/ui/subtitles.test.svelte.ts` passed 55 tests. The additional
`tests/ui/account-library.test.ts` passed 4 cases, including explicit import at
384 and 1280 CSS pixels with 44-pixel controls and no horizontal overflow,
cleared account-switch confirmations and retired native replies. The overlay
file passed 33 tests, including a late subtitle reply after account retirement.
The first new import test needed an awaited Svelte update after cancellation;
its subsequent run passed without weakening the confirmation assertion.

The Windows `cargo check -p video-translate-overlay --features app --bin
video-translate-overlay` passed. After rebuilding the contract host, the focused
library contracts passed 5, browser contracts passed 7, and job contracts passed
9. The job response allow-list now permits and type-checks the public generation
counter; identity and credential fields remain forbidden. An earlier browser
run left a cookie fixture on disk, then passed with added failure diagnostics;
no cause is claimed. Restart runs also exposed a missing job once and a partially
streamed fixture notification. The contract host now buffers each notice before
appending it and reports cookie-file removal errors. The final job-file run
passed all 9 cases; this does not establish a cause for the earlier missing job.

The actual `node scripts/live.mjs --stand-ins` command built and served the
working sources based on `64fdff9`. Its owned backend process was checked before
mutations. A Chromium flow at 384 by 854 passed 9 API/UI assertions for creating
a playlist and adding an untranslated link, with no job request or horizontal
overflow. The owned server and launcher were closed afterward. The embedded
base revision identifies an in-progress working tree, not a stamped native
acceptance candidate. Local screenshots and its receipt remain in ignored
`artifacts/account-library-live-20261115/`.

The cleanup audit kept all affected account, storage, job, browser, screen and
overlay checks. New checks cover distinct request ownership, consent, background
job ownership and presentation boundaries. No tests, skips or expected failures
were removed. The registered native scenario now passes the captured account
counter to site-page calls; `node --check scripts/verification/library-scenario.mjs`
passed, but its updated native scenario has not run.

The thirty-day policy passed `cargo test -p video-translate-overlay --lib
account:: -- --nocapture` (12 tests), `cargo test -p video-translate-overlay
--lib sync_channel:: -- --nocapture` (7 tests), and `cargo test -p
video-translate-overlay --lib google_ -- --nocapture` (25 matching tests).
Historical test-signed enrollments restore without a Google key fetch after
their ID proofs expire; established pairs exchange data over actual loopback
mutual TLS. Unknown certificates, expired trust receipts, mismatched accounts
and expired live-channel authorization fail closed. Local data stays accessible
after device-grant expiry, and an account switch retires its old library scope.
These are offline fixtures, not a thirty-day wall-clock or native-device run.

The test cleanup audit retained all current authentication, account, TLS and
channel tests. The repeated lower-level mismatched-account assertion was folded
into the stronger existing socket-transfer rejection test; no test functions,
skips or expected failures were removed. Unused fixture code for the replaced
one-hour app-session expiry was removed; strict Google-proof expiry checks remain.

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

## Playback implementation checks

The playback API regression passed with `cargo test -p video-translate-overlay
--lib playback_requests -- --nocapture`: one test. It transfers a completed
translation and position into a second journal, reopens its store, checks exact
cue times and 12.5 KRW cost, rejects invalid positions and an advertisement-sized
duration, and refuses writes from a retired account. Repeated positions append
no new journal record. Signed-out video deletion removes its stored position.
The deletion/restoration page regression also passed: one test, covering both
a restored link and a restored completed translation after local file cleanup.

The rebuilt contract host passed `cargo test -p vto-contract-tests --test playback
-- --nocapture` with `VTO_CONTRACT_HOST` naming that binary: one test. Remote P7
get/set persists across restart and refuses deleted videos; settings/library
remain forbidden to site callers, with zero model, Gemini and media calls.
`cargo test -p video-translate-overlay --lib commands:: -- --nocapture` passed
three tests, including unchanged Windows/Android site capabilities.

Chromium playback, overlay, iframe and repeated-injection files passed 42 checks;
the hidden-mount file separately passed one after its route assertion was updated
to allow the single new playback read. The wider initial run failed that old
assertion. Multi-file runs also reported a browser/Vite shutdown timeout; the
42-check rerun exited zero but is not evidence of clean test-runner shutdown.
`npm run build` passed. Windows app `cargo check` passed after supplying the
existing `LIBCLANG_PATH`; the first attempt lacked that environment setting and
failed before checking application code. These are focused implementation checks,
not native playback acceptance or a repository-gate result.

After the repeated-import offer repair, `cargo test -p video-translate-overlay
--lib legacy_ -- --nocapture` passed four affected storage/account checks.
They include a later unowned playlist, retained playback position, deletion
non-resurrection, explicit account ownership and malformed-source rollback.
`npx vitest run -c vitest.ui.config.ts tests/ui/account-library.test.ts` passed
four consent, ownership and layout cases. The cleanup audit extended the
existing import regressions; it removed no current checks.

Saved-version selection extended the existing concurrent-completion regression
instead of adding a duplicate retention test. `cargo test -p
video-translate-overlay --lib sync::library::tests -- --nocapture` passed all
13 projection/import checks. Selecting either response converges its exact cues
and cost across two reopened stores; unknown IDs, active jobs and deleted videos
are refused. The production playback/account regression also exercises P11
selection and rejects it after account retirement. That focused test passed.
`npx vitest run -c vitest.ui.config.ts tests/ui/account-library.test.ts
tests/ui/library.test.ts` passed 19 cases, including measured 44-pixel version
controls without horizontal overflow at 384 and 1280 pixels. `npm run build`
passed. Changed text was checked as valid UTF-8 without BOM; API JSON parsed.

## Native build readiness at fc0d072

Both native builds use application and test source commit
`fc0d072f78248e120db7a7582852add873814cec`. Subsequent documentation-only changes
can describe these results without claiming a new binary or execution.

Windows `cargo build -p video-translate-overlay --features app --bin
video-translate-overlay` completed with the existing libclang installation.
The executable SHA-256 is
`a4f7a5e20f28048de59b4e89757757baf0891093a1c3521c6b8f436576cb6065`.
The owned isolated older app was identified by its running core/screen revisions
before shutdown. Its replacement runs with the same disposable data/WebView
profile; both core and screen report `fc0d072`. The settings screen visibly
reports expired authentication, account-library reads succeed, `sync_ready` is
false, and the connection state is `authentication_required`. No TCP listener
was observed for that app process. Fresh certificate-bound sign-in is pending;
the previous one-hour credential cannot establish a native peer session.

Android's Rust build completed with `cargo tauri android build --debug --apk
--target aarch64 --features app`, then Tauri encountered the documented Windows
symlink restriction. The initial manual fallback used the wrong working
directory and is not candidate evidence. After correcting absolute paths, the
fresh library copy was hash-checked and `assembleArm64Debug -x rustBuildArm64Debug
--console=plain` succeeded. The empty mistaken build directory was removed.
The resulting debug APK is 488,279,623 bytes, SHA-256
`d62207ef970f28d65e6415b1a2756b46ab54a3e7fa74c38b93d7de4b9e0bf2df`.
Its packaged ARM64 core matches the freshly stripped output and contains the
candidate revision. All 962 APK entries were checked for exclusion of the
publisher Desktop secret. `apksigner verify --print-certs` retained certificate
SHA-256 `fa6da2233c2062b48baf20b3562eacacce774da80f2e4bb3d20ec3a6a527ec28`.
These debug artifacts are local test outputs, not a release publication.

The final-source unit-test executable was also run directly with filters
`store::` (5 passed), `sync_runtime::` (7 passed), and
`pipeline::worker_tests::a_job` (2 passed), avoiding the occupied Cargo build
lock. The rebuilt default-feature contract host then passed
`cargo test -p vto-contract-tests --test library --test playback -- --nocapture`
with `VTO_CONTRACT_HOST` naming that binary: 5 library and 1 playback tests.
No full gate, CI or final verification was dispatched.

Private preflight receipts and the expired-state screenshot are retained under
ignored `artifacts/native-sync-20261115/`. No phone was connected during that
preflight. The subsequent installed-device observations below supersede that
readiness-only status.

## Native one-account flows at fc0d072

Windows ran the executable above in an owned disposable application and WebView
profile. Android ran the matching debug APK on a physical Samsung SM-G996N
(Galaxy S21+, Android 15, API 35), installed over the older app without uninstalling.
The original phone had three completed translations and no active jobs. Every
original root-database table row and the sealed Gemini credential's hash remained
unchanged after the update and scenarios. Its existing library was not imported
into the Google account. This is not a Galaxy S21 Ultra or performance run.

The phone initially ran `31ced1f`, which implemented identity setup but not native
LAN synchronization. After installing `fc0d072` and renewing its Google sign-in
to enroll a certificate-bound key, both actual native screens reported connected.
Process restarts restored the thirty-day enrollment without interactive sign-in.
No thirty-day elapsed-time device observation is claimed.

Actual screen actions created two playlists and two untranslated links in both
directions. Playlist delivery took 596 ms from Windows and 5,162 ms from Android;
link delivery took 2,032 ms and 1,999 ms respectively. Local SQLite, local API
responses and visible remote library rows agreed. No address or manual sync
button was used. A concurrent Android rename and Windows reorder of the same
playlist converged with both fields preserved.

Two generated completed responses were inserted into the respective owned local
journals, then projected and transferred by the native controller. This fixture
insertion replaces transcription and translation, not the network or database
projection. Both devices retained exact cue intervals `[[0,1000],[1000,3000]]`,
original text, translated text, glossary and recorded costs of 12.5 and 18.5 KRW.
An alternate completed response at 23.5 KRW remained available after selecting
the original 12.5 KRW response through the phone's version controls. Both devices
converged on that selection without changing its cues or recorded cost.

A private fixture served a generated three-second black video. The Windows
player saved 1,250 ms and the phone resumed there when opened. A subsequent phone
seek to 2,000 ms reached Windows storage without moving its already-controlled
player. Reopening the Windows player resumed at 2,000 ms. The remote Android
page could not call settings, Google-account or library commands. No video
detection or retranslation action was invoked, and no real model or Gemini
translation request was needed. The owned fixture server was stopped afterward.

With Android on Home and its synchronization foreground service active, a
Windows playlist rename reached phone SQLite in 2,123 ms. The observer used a
separate local debug SQLite helper instead of foreground WebView IPC. The phone
was USB-charging. This measures Home delivery, not deep Doze or screen-off
delivery. Screen-off acceptance remains pending.

Windows deletion cancellation preserved the saved video. Confirmed deletion
while Android was force-stopped removed that video's two playlist memberships,
preserved the other translation and playlists, and refused a stale P6 request
without fresh deletion confirmation. A queued rename and deletion survived a
clean Windows shutdown and restart. Closing the owned native windows exited
zero in 8,350 ms and left no old process listener. Reopening Android caught up
with the queued changes and retained the unrelated response's cues and 18.5 KRW
cost. No delivery to a force-stopped application is claimed. A later confirmed
Android deletion reached Windows in 2,307 ms, removed both memberships and made
subtitle reads return `410`, while retaining unrelated links and playlists.

The PC's Ethernet address was `192.168.0.3` and the phone's Wi-Fi address was
`192.168.0.2`. Connections to the actual app listener were observed between
those addresses. USB tethering also supplied another reachable interface during
these scenarios, so the receipts do not prove that every transfer used only
Ethernet and Wi-Fi. Acceptance with tethering disabled, two real Google accounts,
screen-off latency and the project's public OAuth configuration remain pending.

Ignored receipts under `artifacts/native-sync-20261115/` include the library,
completed-response, playback, version, Home, offline, restart and online-deletion
flows, original-data retention checks and native screenshots. They identify
`fc0d072`; later repairs must identify their own sources and builds. These are
focused manual diagnostics, not registered collection receipts, a repository
gate result or final verification.

## Retained cost regression repair

Reviewing version selection exposed two related cost regressions: selecting a
cheaper older response could lower the retranslation baseline, and adding new
usage to a synced response could erase its carried cost because original token
counters are not transferred. The repair keeps the maximum known cumulative
cost and adds each new call's existing unit-price calculation. It does not sum
overlapping cumulative histories or change prices, models or the per-hour cap.

The existing concurrent-version and account/playback regressions were extended
to cover unprojected retained responses, selection of a cheaper version, API
cost-limit refusal, retry from a stale snapshot, additive usage and invalid
usage rejection. The first storage run failed because the carried baseline was
lost during usage accounting; fixing that shared accounting path made all
13 `sync::library::tests` cases pass. The focused `playback_requests` test passed.
`cargo test -p video-translate-overlay --lib pipeline:: -- --nocapture` then
passed all 13 cases in 177.35 seconds, including the deliberate HTTP 503 retry
backoff. An earlier session's final output was unavailable after compaction;
this rerun retained its complete local log instead of inferring a pass.

After rebuilding `vto-contract-host`, `cargo test -p vto-contract-tests --test
translation --test library --test playback -- --nocapture` with
`VTO_CONTRACT_HOST` naming that binary passed 5 translation, 5 library and
1 playback cases. These use stand-ins and made no paid translation request.
The cleanup audit retained the extended storage/account regressions and the
affected pipeline/contract checks; none covers a removed path or duplicates
the retained-version admission boundary. No tests were removed. Native binaries
from `fc0d072` predate this repair; their observations above remain identified
by that source revision. The server-owned gate has not been run here.

### Native readiness after the cost repair

Both native builds and installed screens identify
`5f899a2c72080208f84db666d83e119aa4fcd788`. Windows built successfully; its
executable SHA-256 is
`a07588146b8cca5b8ea6afcb53a479a9fbf99ef5c0ee0ccccc55f9c532abb0f5`.
Android's Rust build succeeded, followed by the known Windows symlink refusal.
The documented absolute-path JNI copy was hash-checked and Gradle packaging
succeeded. Its 488,147,727-byte APK has SHA-256
`e64fc81d8d99045e5150ce8b828f6f36a94373face41caa18eeaf5e3d1d27fd7`.
The packaged stripped core matches the fresh output, all 962 APK entries exclude
the publisher Desktop secret, and the signing certificate matches the existing
installation. Local debug outputs are not published releases.

The Windows readiness harness initially used backslashes for the existing
disposable profile, producing a different path digest from the earlier
forward-slash launch. Both paths resolved to the same directory. Restoring the
original launch value restored the expected digest; the failed assertion was
not removed. The owned process was closed normally before relaunching.

After the in-place APK update, both native cores and screens matched the repair
commit, restored sign-in without interaction, retained the two fixture playlists
and two untranslated links, and reported connected. Actual UI renames reached
the opposite device's SQLite library and visible screen in 1,864 ms from Windows
and 1,974 ms from Android. The pre-update APK backup matched the earlier hash.
All three original phone translations and the sealed Gemini credential remained
unchanged, with no active job. Private receipts are `cost-*-readiness.json`,
`cost-transfer.json` and `phone-after-cost-repair/retention.json` under the same
ignored artifact directory. The topology, screen-off, second-account and public
OAuth limitations above still apply. A documentation-only descendant may cite
these results without claiming rebuilt binaries for that descendant.

### Screen-off delivery over Ethernet and Wi-Fi

After the owner disabled USB tethering, the physical phone had only its
`192.168.0.2/24` Wi-Fi interface for the private LAN; no `usb0` interface
remained. The PC's `192.168.0.3` address belonged to its Ethernet adapter.
The native apps still identified `5f899a2`. USB remained connected for charging
and local diagnostic reads, not as a synchronization network interface.

The phone went to Home and its screen was turned off. After 20,527 ms, a Windows
UI playlist rename reached the phone's SQLite library in 2,695 ms. A second
rename after another ten seconds arrived in 702 ms. The display stayed `OFF`
and interactive mode stayed disabled throughout the 35,012 ms observation.
The native `SyncService` remained foreground with ongoing notification 2002.
The observer read SQLite through the separate local debug helper without
WebView IPC or foregrounding the Android app.

The initial harness incorrectly required wakefulness `Asleep` and stopped
before any mutation: this Samsung device reports `Dozing` with its display
off. The corrected check requires a noninteractive power state and separately
requires display `OFF`; the receipt retains the actual `Dozing` value. No
deep device-idle, unplugged battery or long-term delivery claim follows from
this USB-charging screen-off run. The private receipt is `screen-off-flow.json`.
Second-account observations follow below. Public OAuth publishing configuration
remains pending.

### Two real Google accounts at 5f899a2

The owner completed fresh Google sign-ins on both native platforms with two
different real accounts. Windows first changed to the second account while the
phone retained the original account. Its second-account library began empty.
The phone reported `account_mismatch`; during a 16,089 ms observation, a new
second-account playlist stayed off the phone, and an original-account phone
rename stayed out of the Windows second-account library. No owner account list
was added to the app.

After the phone selected that same second account, the Windows playlist arrived
automatically. Its library contained only that account's playlist, without the
first account's rows. Actual UI renames then traveled from Android to Windows
in 581 ms and from Windows to Android in 2,356 ms over the reachable LAN. Both
native connection states became `connected`.

Windows sign-out released its native listener before the next sign-in opened
its temporary loopback callback. Android sign-out stopped `SyncService` before
the next account began. Reads carrying retired account generations returned
`409 account_changed`. Restoring the original account on both devices retained
its two fixture playlists and links, caught up with its queued phone rename,
and kept second-account rows absent. Direct SQLite observations verified two
distinct account stores on each device and preserved the second account's
playlist after switching away. Retired second-account reads were also refused.

The original phone library still had all three completed translations, identical
original table rows and the same sealed Gemini credential hash, with no active
jobs. The original library was not imported. No real transcription or paid
translation was requested. Both devices were left signed in to the original
Google account.

Private receipts include `account-two-windows.json`, `account-mismatch.json`,
`account-two-phone.json`, `account-two-transfer.json`, `account-restore-*.json`,
`account-restored-flow.json`, `account-store-audit.json`, and
`phone-after-account-switches/retention.json`. The diagnostic Android CDP forward
was recreated after USB reconfiguration removed it; that initial connection
failure occurred before the isolation mutations. Account addresses and tokens
are not included in the public evidence record. The owner reported the Google
project as `External` / `Testing`; production publishing remains a separate
owner action. No repository gate or final verification was dispatched here.

Final native state checks left both devices signed in to the original account
and connected, with matching core/screen build identities. The phone still
offered an explicit original-library import. The account sign-out controls
measured 44 CSS pixels high without horizontal overflow at the observed
384-by-853 phone and 1920-by-1017 Windows viewports. Private viewport screenshots
hide the account address in the DOM before capture; Android full-page coordinate
masking was unsuitable for redaction and those captures were replaced. No
privacy style was retained in the app after capture.

The final test cleanup audit covered 34 changed or affected test/harness source
files, including inline store/command checks and the existing translation,
library, pipeline, UI and overlay regressions. Protocol validation, convergence,
projection, account retirement and native transport checks guard different live
boundaries. Historical receipt compatibility is still supported. No obsolete
skips, removed formats, superseded checks or duplicate observations warranted
removal; all tests were retained. The final-source focused cost checks above
remain applicable because these subsequent changes only update documentation.

Both copies of the owned phone SQLite diagnostic helper were hash-checked
against the local build and removed from the app-private and temporary paths.
Application databases and credentials were retained. The labeled fixture rows
remain available for inspection in their separate Google-account libraries;
none belongs to the original three-translation device library.
