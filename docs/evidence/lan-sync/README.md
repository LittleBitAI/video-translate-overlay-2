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
Documentation-only descendants retain the same application inventory; they do
not acquire a newly stamped binary identity.

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
