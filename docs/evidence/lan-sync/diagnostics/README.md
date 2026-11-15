# Retained native diagnostic sources

These source views make the historical device assertions inspectable. They are
data for review, not scripts to execute against the current user's installation.
The earlier runs remain reused observations. This repair does not claim a new
playback, deletion, screen-off or interactive two-account execution.

The views were collected from the retained local diagnostics during review
round 2. [The source manifest](source-manifest.json) records their current local
hashes and public-view hashes. Normalization changes line endings, the local
home prefix and the hardware serial only. Assertion logic is preserved. These
are not boot-time hashes or independently certified recordings. Some retained
helpers use output basenames from later acceptance runs; that does not change
their checks. The catalog binds the public views and the retained producer
outputs separately so neither can change unnoticed.
Audit-file hashes use UTF-8 text with canonical LF line endings, matching Git's
stored files even when a Windows working copy uses CRLF. Executable and APK
hashes elsewhere in the packet remain SHA-256 of the original binary bytes.

## Required flows

| Flow | Action and assertion sources | Retained machine output |
| --- | --- | --- |
| Playback | [Player actions and bounded assertions](playback-flow.mjs.source), [public fixture](expected-windows.json) | [Original emitted result](../machine/playback-flow.json) |
| Deletion and restart | [Cancellation and offline deletion](offline-prepare.mjs.source), [reopened data assertions](restart-flow.mjs.source), [online deletion](online-delete-flow.mjs.source), [owned process close](close-windows.py.source) | [Offline preparation](../machine/offline-prepared.json), [Windows reopen](../machine/windows-restart.json), [Android reopen](../machine/android-restart.json), [online deletion](../machine/online-delete-flow.json), [process exit](../machine/windows-shutdown.json) |
| Screen-off delivery | [Power state, UI rename and foreground-service assertions](screen-off-flow.mjs.source), [separate SQLite observer](phone-query.py.source), [observer implementation](sql-fixture.c.source) | [Original power-state and latency result](../machine/screen-off-flow.json) |
| Two-account isolation | [First account capture and Windows switch](account-two-windows.mjs.source), [mismatched-account polling](account-mismatch.mjs.source), [phone switch](account-two-phone.mjs.source), [second-account transfers](account-two-transfer.mjs.source), [restore](account-restore.mjs.source), [restored queue](account-restored-flow.mjs.source), [both private stores](account-store-audit.py.source) | [Windows state sequence](../machine/account-two-windows.json), [mismatch result](../machine/account-mismatch.json), [phone state sequence](../machine/account-two-phone.json), [matching transfers](../machine/account-two-transfer.json), [Windows restoration](../machine/account-restore-windows.json), [Android restoration](../machine/account-restore-android.json), [restored queue](../machine/account-restored-flow.json), [store audit](../machine/account-store-audit.json) |

The playback producer records successful-check markers after awaited player
predicates and assertions; it does not retain every raw player sample. Its
1,250 ms and 2,000 ms resume predicates allow 150 ms tolerance. The public
source exposes that tolerance and the throwing timeout, rather than claiming
exact frame-level measurements from the marker text. Deletion producers retain
the checked outcome fields and reopened cue intervals. Screen-off results
retain the actual latency measurements and power-state fields after asserting
the separate device observations. Account producers retain state sequences,
counts or outcome fields after the identity, library and SQLite assertions.

## Inputs and reproduction boundaries

The completed translations are generated test data, not the owner's lectures.
[The seed producer](seed-completed.py.source) shows the exact journal insertion;
[Windows](expected-windows.json), [Android](expected-android.json) and
[alternate response](expected-alternate-windows.json) fixtures expose the cue,
glossary and cost inputs. No speech recognition or paid translation produced
those fixture records. The generated three-second media fixture is already
tracked in the repository. Device source/build identities and observed platform
versions are in the parent receipts and execution catalog.

The original sequence used a disposable Windows application/WebView profile
and labeled account-library fixtures on the owner-approved phone. Playback and
deletion started with both seeded responses in two playlists. Screen-off
observation used the then-single account scope and a local debug SQLite helper;
it did not keep a WebView observer active. The helper was subsequently removed.
Two-account checks ran after those scenarios with two labeled links and playlists
in the original account and an initially empty second-account library.

The first account producer captures a random salt, identity digest, selected
scope and baseline library in a private session file. Later producers compare
against that captured session and the user-selected second account. They do not
contain an owner-account allowlist. That private session file, actual identities,
credentials, browser profiles and original device databases are deliberately
excluded. Inspecting these comparisons requires the code and the state/output
records above, not disclosure of the salt or account address.

Reproducing the native actions requires prepared disposable fixture state,
matching installed builds, an owned Windows process and two user-assisted real
Google sign-ins. Reusing the old literal target identifiers against today's
library would be incorrect. These archived views therefore are not registered
as automatic replay commands. The bounded current native probe remains the
supported collection command for its documented playlist-transfer scope. Its
round-2 attempt refused before app mutations because no phone was attached;
[the device inventory](../machine/round-two-adb-unavailable.log) records that
limit. No missing device run is converted into a pass by the audit replay.
