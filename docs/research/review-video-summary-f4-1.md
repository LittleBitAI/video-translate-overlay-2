# Review research — F4, cycle 1

Draft research before the fix of review round 4 of PR #23, written 2026-11-21. It is not a verified fact or a shared rule: the review reads it with the fix.

## Problem

F6, inferred (I did not have this round's reviewer text): the required runtime evidence (report.json from scripts/verification/video-summary.mjs plus the focused command output) is not tied to the head the review reads. The invariant: the reviewer approves only a head whose API/browser evidence was produced at that exact head and reaches the reviewer in the round's own packet. Something the owner asserts in a reply, or a record that describes an earlier commit, is not that evidence.

Trigger: recurrence, rounds 1, 2, 3, 4.

- [P1] [P1] docs/evidence/video-summary/receipts/ee936b1/README.md:5 — The packet still lacks the comparison that ties the receipts to `bd36588`

## Failed fixes

- Round 1: report.json and the README section were made at 70f30c8 and committed afterwards, so they describe 70f30c8, not the head under review. The README itself says 'A record cannot sit inside the commit it describes' (docs/evidence/video-summary/README.md:116-117).
- Cycle 1 (F6-1 Result): put report.json and the focused outputs inline in the round reply, 'generated at the pushed head named there'. That makes the evidence the owner's prose, not something in the packet. Later commits then moved the head away from it.
- 39c775e -> fdb5179: committed the receipts of 39c775e in fdb5179, with tree ids. The reviewed head (fdb5179) is not the tested head (39c775e). The tree-id argument says they are the same, but a reviewer has to recompute it, and it breaks as soon as anything outside the evidence folder changes.
- ee936b1 -> bd36588: the same approach, extended to the id of every top-level path. The receipts' own rule is 'git diff --name-only ee936b1 <head> must list only files under docs/evidence/video-summary/'. At the current head f8b93f0 that rule already fails: the F2 research commit added a page under docs/research, whose recorded id is 883c1b1f..., and the coming F2 fix will change src-tauri/src/summary.rs. So the receipts go stale again before the next review reads them.
- Common pattern: every fix commits the evidence after the tested commit. That commit, and any later research or fix commit, becomes the reviewed head, so the evidence always describes a predecessor.

## Sources

### S1 — Pro Git, 10.2 Git Internals - Git Objects

- URL: https://git-scm.com/book/en/v2/Git-Internals-Git-Objects
- Retrieved: 2026-11-21
- Locator: Git Objects intro; Tree Objects; Commit Objects
- Fragment: 'This is the SHA-1 hash — a checksum of the content you're storing plus a header'. 'A single tree object contains one or more entries, each of which is the SHA-1 hash of a blob or subtree with its associated mode, type, and filename.' A commit 'specifies the top-level tree for the snapshot of the project at that point; the parent commits if any ... the author/committer information ... and a timestamp; a blank line, and then the commit message.'
- Applicability: A receipts commit cannot record its own id, and any change outside the evidence folder changes the top-level tree and the commit id.
- Counterevidence: The same property makes subtree-id equivalence checkable, which is what the receipts table relies on.
- Validation: The rev-parse and diff checks in the experiment.

### S2 — in-toto Attestation Framework, Statement v1

- URL: https://raw.githubusercontent.com/in-toto/attestation/main/spec/v1/statement.md
- Retrieved: 2026-11-21
- Locator: Fields: subject
- Fragment: subject: 'Set of software artifacts that the attestation applies to.' 'Each element MUST have digest set.' 'the artifacts identified by the subject SHOULD NOT change.' 'Subject artifacts are matched purely by digest, regardless of content type.' Name is optional, e.g. 'a vulnerability scan attestation might leave name unset'.
- Applicability: Evidence names the exact digest it covers and lives outside that artifact. Here the digest is the reviewed commit sha that the done-report and GET /build carry.
- Counterevidence: It is a supply-chain spec for signed attestations; here it is applied by analogy to an unsigned report.
- Validation: The report's head equals the PR head and the GET /build head.

### S3 — git-notes(1) manual (MIT mirror)

- URL: https://web.mit.edu/git/git-doc/git-notes.html
- Retrieved: 2026-11-21
- Locator: DESCRIPTION; EXAMPLES
- Fragment: 'This command allows you to add/remove notes to/from objects, without changing the objects themselves.' The example: git notes add -m 'Tested-by: ...' 72a144e2. (Quoted through a search summary; older mirror, flags may differ in current Git.)
- Applicability: Git's own way to attach a test record to an existing commit is external to that commit, which confirms that the in-commit receipts pattern works against the design.
- Counterevidence: The app's review packet does not read notes, so this is not the fix here.
- Validation: None needed.

### S4 — GitHub REST API, Commit statuses

- URL: https://docs.github.com/en/rest/commits/statuses
- Retrieved: 2026-11-21
- Locator: About commit statuses; Create a commit status
- Fragment: 'Users with push access in a repository can create commit statuses for a given SHA.' Statuses mark commits 'error, failure, pending, or success' with optional description and target_url. The combined state is pending 'if there are no statuses'. Limit: 'there is a limit of 1000 statuses per sha and context within a repository.'
- Applicability: CI binds a result to one sha, and a new commit starts with no status. That matches the reviewer seeing no evidence for the moved head.
- Counterevidence: The app, not GitHub statuses, owns review evidence here.
- Validation: None needed.

### S5 — Receipts at ee936b1

- URL: docs/evidence/video-summary/receipts/ee936b1/README.md
- Retrieved: 2026-11-21
- Locator: line 3; table row docs/research; final block lines 118-121
- Fragment: 'Made after the commit they describe ... at a later head each must match git rev-parse <head>:<path> ... git diff --name-only ee936b1... <head> must then list only files under docs/evidence/video-summary/.' Row: docs/research 883c1b1fb42f4cd85890ede1fb200f672f25aefb. '21/21 checks passed at head ee936b1...'
- Applicability: This is the current binding, and its own condition fails once f8b93f0 adds a research page.
- Counterevidence: It is a reasonable and checkable argument; the reviewer may simply not have accepted it.
- Validation: The diff check in the experiment.

### S6 — Video summary execution evidence

- URL: docs/evidence/video-summary/README.md
- Retrieved: 2026-11-21
- Locator: lines 114-124
- Fragment: 'A record cannot sit inside the commit it describes. So a later record is made with the same script and commands at a pushed head, then committed under receipts/<head>/ in a commit that changes only evidence files'.
- Applicability: This states the after-the-fact pattern that each round repeated.
- Counterevidence: none
- Validation: The next fix drops or adjusts this paragraph to say the evidence goes with the report at the final head.

### S7 — Review research, F6 cycle 1

- URL: docs/research/review-video-summary-f6-1.md
- Retrieved: 2026-11-21
- Locator: line 11 finding; Result line 124
- Fragment: Finding: '[P1] docs/evidence/video-summary/report.json:3 — The generated receipt and focused output are missing from the round packet'. Result: 'For the missing receipt, the round reply carries report.json and the focused outputs inline, generated at the pushed head named there.'
- Applicability: This is the finding's wording and the earlier fix that put the evidence in reply prose.
- Counterevidence: The body of that page researched the next_topics language check, so the mapping from finding to page is unclear.
- Validation: The next report carries the outputs at the exact head.

## Claims

- C1 (source; S1): A commit's id is a hash over its content, including its top-level tree, whose entries hash every blob and subtree. So a commit cannot contain a record of its own id, and any changed path changes the id.
- C2 (source; S2, S3, S4): The established ways to tie verification results to a commit attach them outside the commit, keyed by its exact digest or sha.
- C3 (interpretation; S1, S5): At f8b93f0 the ee936b1 receipts' own validity condition no longer holds, because a path outside docs/evidence/video-summary/ (docs/research) changed. The coming F2 fix would break it again.
- C4 (interpretation; S5, S6, S7): Each earlier fix ran the evidence at one head and delivered it in a later commit or in reply prose, so the reviewed head never had evidence produced at it.
- C5 (interpretation; S2, S4, S7): The fix that should hold is to take the evidence last, at the final pushed head, and file the done-report there with the outputs inline, with no commit after it.

## Core theory

Git content addressing: an object's id is the SHA-1 of a header plus its content. A commit names its top-level tree, and each tree entry is the hash of a blob or subtree. So a commit's id depends on everything it contains, and evidence about a commit has to sit outside it. The established ways to attach a record to an existing commit are all external: git notes attach to objects 'without changing the objects themselves', GitHub commit statuses are created 'for a given SHA', and in-toto attestations name their subject by digest, matched 'purely by digest', and the subject 'SHOULD NOT change'.

## Applicability

In this runtime, the done-report filed through the app plays the role of an external record bound to the head (like a status or an attestation), and there is no registered execution catalog. The repo-side mistake is in docs/evidence/video-summary/README.md:114-124 and receipts/ee936b1/README.md:3, which define the evidence as a committed file about an earlier head. The fix belongs in the order of the next fix turn, not in summary.rs. The F2 fix (summary.rs, tests, backend.md) and the F2-1 Result go in one commit. Push. Run the gate's focused commands and node scripts/verification/video-summary.mjs against live.mjs --stand-ins, whose GET /build must answer that sha. File the done-report at that sha with the outputs, and stop. Any committed receipts must be in that same commit, or else the run is repeated at the receipts commit before the report.

## Hypothesis

The finding keeps coming back because the evidence is produced at head N and then delivered in commit N+1 or later. A commit's id hashes its tree, so no commit can contain a record of itself. The tree-equivalence workaround asks the reviewer to recompute ids, and any change outside the evidence folder breaks it (the research page, the F2 fix). The host shows the implementation report as current only when it was filed at the reviewed head. Since the head moved after every report, the reviewer had no current-head evidence. A fix that holds: make no commit after the evidence is taken. Commit everything, including the research Result, code, tests, spec text and (if they are wanted) the receipts files. Push. Run the focused commands and the verification script at that final pushed head, with GET /build reporting that same sha. File the done-report at that head with the machine output inline (command output, '21/21 checks passed at head <sha>', report.json sha256).

## Counterevidence

- I am inferring which finding this is. The latest commit is the F2 cycle 1 page, so this round could also be F2 again (the said_whole spaceless-script bug), which this page does not address.
- The tree-id equivalence is sound in principle (Pro Git: tree entries hash their contents). A reviewer with read-only git could check it, so the refusal may have been about the packet (the outputs were not in the done-report) rather than about head binding.
- The host's rule that only a report filed at the reviewed head counts comes from reading the host's review-contract code outside this repository. It cannot be cited here, and it may have changed.

## Alternatives

- Keep committing receipts/<head>/ and extend the tree-id table to cover docs/research: this patches the newest example again. The F2 fix changes src-tauri, so it can never cover the head that holds the fix. Rejected.
- git notes or a GitHub commit status on the tested sha: both are the textbook external binding, but the app's review packet does not read either, and the review loop is owned by the app. Not first.
- Run the evidence at the receipts commit itself (commit the receipts, then run again at that sha and report there): this works and keeps committed records, but it costs a second run. Use it only if a done item requires committed receipts.

## Experiment

Before editing, confirm the stale binding at the current head: `git diff --name-only ee936b1056e24542a85b3b48b698067eb2cb83db f8b93f0` should list docs/research/review-video-summary-f2-1.md, a path outside docs/evidence/video-summary/, and `git rev-parse f8b93f0:docs/research` should differ from 883c1b1fb42f4cd85890ede1fb200f672f25aefb. After the fix: `git rev-parse HEAD` must equal the PR head from gh pr view --json headRefOid, must equal the head in GET /build during the verification run, and must equal the head the done-report was filed at. No commit may follow the report. If the next review still refuses with that equality holding, the hypothesis is refuted, and the finding is about the evidence's content rather than its binding.

## Result

pending — the fix records what it observed here, with the tests and the commit.

## Unresolved limits

I did not have this round's reviewer text, and I could not run git or a shell, so the predicted results of the diff and rev-parse checks have not been observed. I don't know how the app builds the round packet beyond the host code I read outside this repo. I don't know whether a done item requires committed receipts. The git notes source is an older mirror of the manual.
