# LAN synchronization execution evidence

This packet makes implementation observations inspectable without access to
ignored artifacts. `verification.json` registers this reference and its
[execution catalog](catalog.json). Receipts contain observed results, environment,
build identities and explicit reuse limits. They are implementation evidence,
not independently certified executions or a replacement for the server gate.

The reviewed starting revision is `ac67b9c110aa235af69647a27fffd89679030a02`.
The repair changes evidence tooling, tests and documentation, not application
sources. The catalog binds each execution to a Git source inventory. Run
`node scripts/verification/sync-evidence.mjs HEAD` to check the candidate against
those inventories, receipt hashes, required evidence kinds and observations.
Any application input addition, removal or edit invalidates full-source reuse.
The validator and collection probes also refuse dirty application inputs before
accepting evidence for a checked-out head. Host-owned wiki metadata and evidence
documentation do not change application sources.
Documentation-only descendants retain the same application inventory; they do
not acquire a newly stamped binary identity.

## Observed executions from review round 1 and earlier

The fresh executions ran on Windows build 26200 with Node 24.18.0 and Rust
1.98.1 against application source `c52830bf29506fc57810067cf6f1f5fba2621e09`.
Chromium was 149.0.7827.55. Native cores and screens identified
`ff30ca80e0acd1af7c0ec110705ae8dc5d94d881`; the complete application inventory
matches the reviewed and repair sources. Windows used WebView2 157.0.4341.46;
the physical Android 15/API 35 ARM64 Galaxy S21+ used WebView 157.0.8103.38.
The measured Windows executable SHA-256 is
`de72186bbfc1691697da60e4f4ab4bfa039072328d3e05e2d4a516345489e847` and the
packaged APK SHA-256 is
`a223f624956fe674bd3a8bd85c596d2f2ba9f51704c224b6ba9e9fa0fbe05416`.
Those identities are recorded inside the receipts, not inferred from filenames.

| Receipt | Execution and observed result |
| --- | --- |
| [Fresh API](fresh-api.json) | 4 assertions: exact cue/cost reuse, restart and deletion, no automatic translation, playlist removal retaining subtitles. |
| [Fresh Chromium](fresh-browser.json) | 6 actual UI scenarios against the real current-source API, including restart, cancellation, confirmed deletion and manual opening. |
| [Fresh live command](fresh-live-command.json) | The actual development launcher served its owned current-source process; HTTP refused native policy opening; its process tree stopped and released the port. |
| [Fresh native transfer](fresh-native.json) | Both cores/screens matched their recorded build; same-account UI renames reached peer SQLite and visible UI in both directions; original test names were restored. |
| [Fresh offline transport and ownership](fresh-offline.json) | 24 passed, including framing, TLS identity, replay, anonymous discovery, bounded retry and resource shutdown. |
| [Fresh projection and cost](fresh-projection.json) | 13 passed, including retained versions, deletion/active-job rules, exact cues/costs and carried-cost safeguards. |
| [Reused completed responses](reused-completed-and-versions.json) | Earlier physical-device observations at `fc0d072`, with both responses and selectable retained versions. |
| [Reused playback](reused-playback.json) | Earlier device resume at 1,250 ms, remote position at 2,000 ms without moving controlled playback, then resume after reopening. |
| [Reused deletion and restart](reused-deletion-and-restart.json) | Earlier online/offline deletion, cancellation, concurrent edits, clean exit and retained data after reopening both apps. |
| [Reused screen-off](reused-screen-off.json) | Ethernet/Wi-Fi only, display off for 35.012 seconds; delivery 2,695 ms and 702 ms, foreground service present. |
| [Reused real-account isolation](reused-two-real-accounts.json) | Two real accounts on both devices; mismatched-account rejection and isolated retained stores, followed by matching-account transfers. |
| [Reused policy and retention](reused-policy-and-retention.json) | Actual external browser destination on both platforms, unchanged account/video state, and original phone rows and credential preserved after update. |
| [Reused public pages](reused-public-pages.json) | Four anonymous HTTPS responses with measured hashes and real browser navigation. |

Older device observations bind unchanged components, not a claim that every
application file is identical to those older commits. The catalog separately
lists every changed application input outside each component inventory.
The intervening cost-accounting repair affects retained-cost admission and
new usage accumulation; the fresh thirteen projection tests cover that boundary.
Later native entry-point and screen changes add the fixed privacy action and
its local permission. The final full-source native transfer observation tests
their integrated application. Raw device databases, account addresses and
credentials are not needed to inspect these observations.

Fresh API and Chromium executions use the registered library scenarios, separate
seeded databases, an owned loopback server and generated media. They check the
running process and compiled revision before mutations. Native observations use
the already signed-in disposable Windows profile and the owner-approved physical
Galaxy S21+ installation. Native evidence records its actual compiled revision
and measured artifacts, rather than relabeling an existing binary as a new build.
Only labeled acceptance playlists are renamed, and their names are restored.
No real transcription, paid translation, credential changes or old-library import
are requested by these checks.

The catalog distinguishes fresh executions, earlier observations reused for
unchanged components, and owner reports. Earlier full-device scenarios are not
fresh runs of this repair. Their component inventories and the separately
documented changed call sites make the reuse boundary inspectable. Raw account
identifiers, credentials, browser profiles, original databases and private
sign-in material are excluded. The public support contact remains on the
approved information site.

Reproduction needs Node dependencies, Rust, Chromium and the documented native
build prerequisites. The existing API/browser flows take
`WIKI_VERIFICATION_HEAD`, `WIKI_VERIFICATION_ARTIFACTS`,
`WIKI_VERIFICATION_ENVIRONMENT` and `WIKI_VERIFICATION_SCOPE` as documented by
their runner. The native probe takes the compiled revision and output receipt
path; `VTO_ADB` selects the installed Android debugging tool. It requires the
owned Windows and Android WebViews on debugging ports 9226 and 9227, already
signed in to the same account. A read-only reviewer can inspect the committed
packet and validate source binding without signing in or modifying a device.

Google's production publishing state is an owner report. Screen-off observations
cover a short USB-charging run, not prolonged deep Doze. Thirty-day expiry is
checked offline, not by waiting thirty days. The server owns the complete gate
and final verification; this packet does not start either or authorize merge.

## Review repair audit

The evidence boundary covers four kinds: API, browser, desktop and offline.
Six executions were collected fresh in review round 1; seven entries reuse
earlier observations. Review round 2 reuses those executions while adding the
previously missing [diagnostic sources and retained machine output](diagnostics/README.md).
It does not claim fresh execution of the four required native flows.

Every catalog entry now needs hashed producer sources and machine output,
inspectable action excerpts and executable assertion excerpts bound to observed
output fields. A named successful observation alone is refused regardless of
whether its result is a boolean, string, object or null. Required playback,
deletion/restart, screen-off and account-isolation assertions are checked
separately; four evidence kinds and a playlist rename cannot substitute for
those flows. Bounded player predicates also expose their throwing timeout path.

All thirteen entries were audited, including the nine outside the four native
flows identified in the finding. File hashes, excerpt availability, observed
output comparisons, required native assertion coverage, source inventories and
dirty application inputs have separate rejection tests. This validates an
inspectable audit trail; it does not independently certify historical execution.

The lifecycle correction replaces the reported unstarted-controller paragraph.
Three additional current-status references were corrected in the backend design,
backend reference and Google setup introduction. Earlier progress notes now sit
under a dated historical section. The documentation regression checks all four
current descriptions and the actual native start/stop call sites; the fresh
runtime tests separately exercise resource ownership and cancellation.

The round-2 cleanup pass extends the original catalog regression rather than
retaining the old pass-summary acceptance fixture. It keeps the distinct source
equivalence, dirty-input and lifecycle checks and adds refusal tables for
pass-only receipts, incomplete native audits and unbounded predicates. No test
is skipped or tied to a removed route. No behavior check was removed.
