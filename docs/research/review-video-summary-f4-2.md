# Review research — F4, cycle 2

Draft research before the fix of review round 5 of PR #23, written 2026-11-21. It is not a verified fact or a shared rule: the review reads it with the fix.

## Problem

F4, summary.review-evidence: the required API and browser observations (environment, plus expected and observed values for each check) and the focused check output must reach the reviewer for exactly the head under review. In round 5 the reviewer read 2c0cc1d. The only machine-readable record they could open, docs/evidence/video-summary/report.json, names head 70f30c8 at line 3. The round's implementation report carried only short PASS excerpts and a SHA-256 of a report.json that was not in the packet. The only server receipt was `git diff --check`. So the current head had no execution record the reviewer could check.

Trigger: recurrence, rounds 1, 2, 3, 4, 5.

- [P1] [P1] docs/evidence/video-summary/report.json:3 — The round omits the execution records for the reviewed head

## Failed fixes

- Round 1: report.json and the README record were made at 70f30c8 and committed later, so they describe 70f30c8. They are still in the tree, and report.json:3 is this round's anchor.
- 39c775e -> fdb5179 and ee936b1 -> bd36588: receipts for an earlier commit were committed together with tree or path ids, arguing equivalence. Any later commit outside the evidence folder broke that argument, and the reviewer refused it in round 4 because the comparison tying the receipts to the head was missing.
- Cycle 1 (bd36588 -> 2c0cc1d, following review-video-summary-f4-1.md): removed the receipts folder, ran everything at the final pushed head 2c0cc1d with no later commit, and filed the done-report there. That fixed the head binding. But the report reduced the run to a few PASS lines, a '21/21 checks passed' line and the sha256 of report.json. It never carried the report's content. The reviewer cannot open a file that exists only on the owner's disk, the excerpts left out the environment and the expected values, and the stale committed report.json stayed readable as if it were current. Cycle 1's hypothesis covered when the evidence is taken, not what reaches the reviewer.

## Sources

### S1 — in-toto Attestation Framework: ResourceDescriptor

- URL: https://raw.githubusercontent.com/in-toto/attestation/main/spec/v1/resource_descriptor.md
- Retrieved: 2026-11-21
- Locator: Fields: uri, digest, content, downloadLocation; minimum requirement
- Fragment: uri: 'This field is REQUIRED unless either `digest` or `content` is set.' digest: 'When known, the producer SHOULD set this field to denote an immutable artifact or resource.' content: 'The contents of the resource or artifact.' 'To maintain size efficiency, the size of `content` SHOULD be less than 1KB.' downloadLocation: 'To enable automated downloads by consumers, the specified location SHOULD be resolvable.' 'a ResourceDescriptor MUST specify one of `uri`, `digest` or `content` at a minimum.' (Quoted through a fetch summarizer.)
- Applicability: A record handed to a consumer either carries the content or points to a location the consumer can resolve. Cycle 1 gave only a digest of a file the reviewer could not reach.
- Counterevidence: The spec allows a digest alone as a minimum, for consumers that already have the artifact. It recommends inline content under 1KB, and the full report is larger.
- Validation: The done-report carries report.json verbatim, and its sha256 matches the digest it states.

### S2 — in-toto Attestation Framework: Test Result predicate

- URL: https://raw.githubusercontent.com/in-toto/attestation/main/spec/predicates/test-result.md
- Retrieved: 2026-11-21
- Locator: Purpose; Fields
- Fragment: 'This predicate type defines a generic schema to express the result of running tests in software supply chains.' 'each attestation corresponds to one invocation of a test suite'. result (required): 'One of `PASSED`, `WARNED`, or `FAILED`.' configuration (required): 'Reference to the configuration used for the test run.' passedTests / failedTests (optional): 'Each entry corresponds to the name of a single test that passed' ... 'The semantics of the name must be determined separately between the producer and consumer.' (Quoted through a fetch summarizer.)
- Applicability: The reviewer contract's 'bound to environment and head' corresponds to the predicate's required configuration. The excerpts cycle 1 sent had the pass result but not the configuration.
- Counterevidence: This is a supply-chain spec for signed attestations, used here by analogy for an unsigned report. Test names are only optional in it.
- Validation: The inline report includes the environment block, and each check's expected and observed values.

### S3 — Committed report.json (round 1 record)

- URL: docs/evidence/video-summary/report.json
- Retrieved: 2026-11-21
- Locator: lines 2-11
- Fragment: "build": { "head": "70f30c897b87e3e8ce16be42e4577ab9ea93290a", "pid": 35636 }, "environment": { "node": "v24.18.0", "platform": "win32 x64", "playwright": "1.61.0" }, "taken_at": "2026-11-20T14:43:51.461Z"
- Applicability: This is the only full execution record a reviewer can open at 2c0cc1d, and it names a different head. It is also the finding's anchor.
- Counterevidence: The README labels it as round 1's record, so a careful reader would not take it as current.
- Validation: At the final head it is deleted or clearly renamed, and the README stops linking it as the record.

### S4 — Video summary execution evidence, record section

- URL: docs/evidence/video-summary/README.md
- Retrieved: 2026-11-21
- Locator: lines 114-122, 135-139
- Fragment: 'This section and [`report.json`](report.json) are round 1's record, made at `70f30c8`. A record cannot sit inside the commit it describes ... So a later record is made with the same script and commands at the final pushed head of a round, with `GET /build` naming that head, and filed in that round's report at that head, with its output inline; no commit follows it.'
- Applicability: The written procedure already says 'output inline'. Cycle 1 filed a summary and a digest instead of the output.
- Counterevidence: none
- Validation: The next done-report follows the paragraph literally, with the full report.json and the raw command output.

### S5 — scripts/verification/video-summary.mjs

- URL: scripts/verification/video-summary.mjs
- Retrieved: 2026-11-21
- Locator: lines 5-6, 117, 122-123
- Fragment: 'Writes <out-dir>/report.json and <out-dir>/<size>-<state>.png; exits 1 if any check fails.' environment: { node: process.version, platform: ..., playwright: ... } ... writeFileSync(join(out, 'report.json'), ...); console.log(`${...}/${checks.length} checks passed at head ${build?.head}`)
- Applicability: The script already produces the full record the reviewer needs. Its console line is the summary that cycle 1 forwarded in place of the file.
- Counterevidence: none
- Validation: The report.json written at H is the one pasted into the done-report.

### S6 — Review research — F4, cycle 1

- URL: docs/research/review-video-summary-f4-1.md
- Retrieved: 2026-11-21
- Locator: Hypothesis; Result; Experiment
- Fragment: 'File the done-report at that head with the machine output inline (command output, '21/21 checks passed at head <sha>', report.json sha256).' 'If the next review still refuses with that equality holding, the hypothesis is refuted, and the finding is about the evidence's content rather than its binding.'
- Applicability: Cycle 1 predicted this outcome. The equality held at 2c0cc1d and the refusal concerned content, so its own refutation clause applies.
- Counterevidence: Its listed 'machine output' items were a summary line and a digest, which cycle 1 treated as enough.
- Validation: n/a (history)

## Claims

- C1 (source; S1): A resource descriptor must give a uri, a digest or inline content. A digest denotes an immutable artifact, and a download location should be resolvable for consumers to retrieve it.
- C2 (source; S2): A test-result attestation requires the run's result and a reference to the configuration used for the run.
- C3 (interpretation; S1, S3, S4): At 2c0cc1d the reviewer could open only a report naming 70f30c8, plus a digest of a current report they could not open, so the current head had no checkable execution record.
- C4 (interpretation; S4, S5, S6): Cycle 1 fixed when the evidence is taken but forwarded the script's summary line and a digest instead of its generated report.json and the raw command output, contrary to the README's 'output inline'.
- C5 (interpretation; S6): By cycle 1's own refutation clause, the round-5 refusal with the head equality holding shows the remaining defect is the evidence's content, which inlining the full report and raw outputs, and retiring the stale committed report, addresses.

## Core theory

A record of a test run identifies the artifact it covers and carries, or points to, its content and configuration. The in-toto ResourceDescriptor must have a uri, a digest or inline content. Its producer should set a digest to denote an immutable artifact, and a download location should be resolvable for automated retrieval, so a digest identifies an artifact without delivering it. The in-toto test-result predicate requires a result and the configuration used for the run, and lists passed and failed tests by name. A bare pass count without its configuration is less than that predicate requires.

## Applicability

Here the 'consumer' is the reviewer, who can read only the packet and the repository at the head. scripts/verification/video-summary.mjs:117-123 already writes everything needed (build head, environment, expected vs observed) to report.json and prints a single summary line. Cycle 1 forwarded that summary line and a digest instead of the file. docs/evidence/video-summary/README.md:114-122 says a later record is 'filed in that round's report at that head, with its output inline'. That intent was right, but the filing used excerpts. Committed report.json:3 (head 70f30c8) is what the reviewer opened. The change goes in the evidence files and in how the done-report is filed, not in summary.rs.

## Hypothesis

The evidence is now taken at the right head, but it does not reach the reviewer. The round packet carries three things: committed files, the done-report's evidence strings and server receipts. The generated report.json for the final head is in none of them. A digest without the content, or without a location the reviewer can resolve, proves nothing to them, and the excerpts drop the fields the contract requires (environment, expected vs observed). Meanwhile the only report the reviewer can read is the committed 70f30c8 one. Fix: at the final pushed head H, with no commit after, run the verification script and the focused commands. Put the generated report.json verbatim (build.head = H, pid, environment, taken_at and every check's what/expected/observed/pass) into the done-report evidence for the API/browser items. Put the raw focused output there too (cargo's per-test 'ok' lines and 'test result' lines, vitest's summary, the build line), not paraphrases. In the commit before H, delete docs/evidence/video-summary/report.json, or move it to a name that marks it as round 1's 70f30c8 record. Update the README's 'Execution record at 70f30c8' section so it no longer presents a committed report as the current record.

## Counterevidence

- The packet labels the implementation report 'untrusted claims' and says a pass flag or owner confirmation is a claim, not certified runtime proof. Full inline JSON is still owner-supplied, so a reviewer might want a server-run receipt. Only the app or the person can provide that, since no execution catalog is registered.
- The done-report evidence field may have a length limit, or the packet builder may truncate it. A full report.json of 21 checks is several KB, and in-toto itself says inline content SHOULD stay under 1KB for size efficiency.
- The reviewer also notes that the only server receipt is git diff --check, and the first done item says 'Server gate pending'. The missing full-gate receipt is the server's to run and cannot be fixed from the fixing side.

## Alternatives

- Commit the newly generated report.json in commit H+1 and run again at H+1. The committed file would still name H, not H+1, because a commit cannot contain a record of itself. That repeats the round 1 to 4 pattern. Rejected.
- Keep the committed report.json and add a tree-id or path-id comparison to the head. Round 4 already refused this, and any later commit breaks it. Rejected.
- git notes or a GitHub commit status on the tested sha. These are external bindings, but the app's packet reads neither. Not first.
- Ask the person to register an execution catalog so the server runs the verification and attaches its own receipt. This is the strongest binding, but it is outside the fixing side's reach. Offer it to the person if inline content is refused again.

## Experiment

At the final pushed head H, with no commit after it: (1) git rev-parse HEAD = gh pr view 23 --json headRefOid = GET /build head during the run. (2) The generated report.json has build.head = H, and the done-report contains that JSON byte for byte, so its sha256 equals that of the file on disk. (3) The done-report carries the raw focused output lines (each named test '... ok' and every 'test result:' line). (4) `git ls-files docs/evidence/video-summary/report.json` is empty at H (the file was deleted), or the file sits under a name that marks it as round 1's history, and the README no longer calls it the current record. Confirmation: the next round's packet shows the full JSON under 'Current-head implementation report', and the reviewer does not raise F4. Refutation: if the packet truncates or omits the inline JSON, the channel is the limit, and the person has to choose a server-side receipt. If the reviewer refuses even with the full JSON present, the objection is that the evidence is owner-supplied, not that records are missing.

## Result

pending — the fix records what it observed here, with the tests and the commit.

## Unresolved limits

No shell in this session: nothing was run, and the report's size was not measured. How the app builds the packet from done-report evidence, and whether it truncates, is not documented in this repository. The round-5 reviewer text was read outside the repository and is paraphrased here, not cited. Whether the reviewer will ever accept owner-supplied inline records as enough without a server receipt is unknown.
