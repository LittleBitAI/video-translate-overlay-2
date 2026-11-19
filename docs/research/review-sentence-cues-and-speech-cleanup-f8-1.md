# Review research — F8, cycle 1

Draft research before the fix of review round 5 of PR #22, written 2026-11-20. It is not a verified fact or a shared rule: the review reads it with the fix.

## Problem

README.md's evaluation totals must equal the verdicts actually recorded per place in places.md. README itself names places.md as the authority: 'The old and new Korean of every place, with its verdict, are in places.md'. For WjhDDeZ7DvM, places.md:121-196 holds 76 rows (45 better, 22 same, 9 worse). README states 75/43/23/9 at line 398, 578/354/170/54 at line 403, and '75 in all. Tally: better 43, same 23, worse 9' at line 511, so the summary contradicts the per-place evidence.

Trigger: recurrence, rounds 1, 2, 4, 5.

- [P1] [P1] docs/evidence/sentence-cues/README.md:398 — The evaluation summary contradicts the underlying per-place judgments, making the required evidence factually invalid.

## Failed fixes

- Round 1 (fix after 3f8c8ed): added places.md with old/new Korean pairs and per-video rendered counts. This fixed the missing evidence, but it created a second copy of the per-place data (places.md rows vs README grouped tables and tallies), and nothing ties the two together.
- Round 2 (fix after 1ff5752): re-read 65 '—' places and 32 'Same.' places, changed four Dory verdicts from same to better and split one vSCYL8ySh9g row into two. Those tallies were updated by hand and are correct today, which shows the totals depend on someone remembering to recount after every row edit.
- Round 4 (fix after 1dcd50f): rewrote the WjhDDeZ7DvM reasons so each row describes its own pair (the 14:18/14:26/14:35 area). The current WjhDDeZ7DvM rows differ from the README by +1 place, +2 better and -1 same. That fits one 'same' row being re-read and split into two 'better' rows during this or a nearby rewrite. The hand-typed README numbers (lines 398, 403, 511) were not recounted. Which commit introduced the drift is unverified: no shell or git access in this session.
- Receipts for round 5 (183b6ca) cite check_readme.py 'differences 0'. That script only compares the rendered-line tables with expected.json and never reads places.md, so it could not catch verdict drift and gave false assurance.

## Sources

### S1 — Don't repeat yourself — Wikipedia

- URL: https://en.wikipedia.org/wiki/Don%27t_repeat_yourself
- Retrieved: 2026-11-20
- Locator: Definition / 'Hunt and Thomas'
- Fragment: "Every piece of knowledge must have a single, unambiguous, authoritative representation within a system." The authors apply it to "database schemas, test plans, the build system, even documentation." Criticism noted: AHA ('avoid hasty abstractions'); Sandi Metz: "prefer duplication over the wrong abstraction."
- Applicability: The verdict counts are knowledge with two representations (places.md rows and the README tallies), and documentation is explicitly in scope.
- Counterevidence: AHA/Metz: duplication can be better than a wrong abstraction. Here the remedy is a check, not an abstraction, and the README summary is kept.
- Validation: After the fix, the README tallies are verified against places.md by the check script on every run.

### S2 — Compute derived values from source fields instead of hand-maintaining them (OpenShrike data-027)

- URL: https://openshrike.networkperspective.ai/library/checks/shared/data/data-027-compute-derived-values-from-source-fields-instead-of-hand-maintaining-them/
- Retrieved: 2026-11-20
- Locator: Rule, rationale and exceptions
- Fragment: Rule: "Compute derived values from source fields instead of hand-maintaining them". Aim: avoid "source-of-truth drift caused by manually synchronizing derived values in multiple places." Not flagged when stored derived values have "one visible authoritative recomputation path and consistency guardrails." "Return `unknown` when persistence constraints require materialization and sync logic is elsewhere."
- Applicability: The README tallies must stay as stored text (materialized), so the exception applies only if a recomputation path and guardrail exist. The existing check_readme.py is that path once it reads places.md.
- Counterevidence: This is a lint rule aimed at code and data models, not prose documentation; it is applied here by analogy.
- Validation: The extended check reports the WjhDDeZ7DvM drift today and 0 differences after the correction; a flipped verdict in places.md makes it fail.

### S3 — places.md — Sentence cues: every place read, old and new Korean

- URL: docs/evidence/sentence-cues/places.md
- Retrieved: 2026-11-20
- Locator: lines 117-196 (### WjhDDeZ7DvM); verdict cells across lines 12-620
- Fragment: WjhDDeZ7DvM rows at lines 121-196: 76 rows; 'worse' at 141, 152, 153, 156, 166, 171, 176, 177, 185 (9); 'same' at 121, 122, 128, 130-134, 148, 161, 168, 170, 174, 178, 182, 184, 191-196 (22); the remaining 45 are 'better'. The whole file has 579 verdict rows, 54 worse and 169 same.
- Applicability: The system of record the README summarises.
- Counterevidence: Could itself contain an unintended split. Not observed: the timestamps and sentences are distinct.
- Validation: Script count equals this hand count.

### S4 — README.md — Reading old and new Korean (done item 15)

- URL: docs/evidence/sentence-cues/README.md
- Retrieved: 2026-11-20
- Locator: lines 373, 393-403, 511
- Fragment: Line 373: "The old and new Korean of every place, with its verdict, are in places.md". Line 398: "| `WjhDDeZ7DvM` | 75 | 43 | 23 | 9 |". Line 403: "| All 8 | 578 | 354 | 170 | 54 |". Line 511: "…every old filler/repeat place (9 F), every misheard-term place (52 T/MT), and 14 sampled mid-sentence places: 75 in all. Tally: better 43, same 23, worse 9."
- Applicability: These are the derived copies that drifted. The other seven summary rows match places.md.
- Counterevidence: none given
- Validation: The check passes after the correction.

### S5 — check_readme.py

- URL: docs/evidence/sentence-cues/measure/check_readme.py
- Retrieved: 2026-11-20
- Locator: docstring lines 1-3; loops at 13-39
- Fragment: "checks that the rendered-line tables in ../README.md are sums of expected.json". It reads only expected.json and README.md, never places.md.
- Applicability: This is why 'differences 0' in the receipts did not validate the evaluation totals, and it is the place to add the tally check.
- Counterevidence: none given
- Validation: After the extension it reads places.md, and a planted verdict change makes it fail.

## Claims

- C1 (source; S1): Each piece of knowledge, documentation included, should have one authoritative representation.
- C2 (source; S2): Hand-synchronised derived values drift from their source of truth.
- C3 (source; S2): A stored derived value is acceptable when it has one authoritative recomputation path and consistency guardrails.
- C4 (source; S3, S4): places.md records 76 WjhDDeZ7DvM places (45/22/9) and 579 overall (356/169/54), while README states 75/43/23/9 and 578/354/170/54. The other seven videos match.
- C5 (source; S4, S5): The receipts' 'differences 0' could not detect this, because check_readme.py never reads places.md.
- C6 (interpretation; S1, S2, S5): The recurrence comes from edits to places.md rows in fix rounds without recounting the hand-typed README tallies. The durable fix is to extend check_readme.py to recount the verdicts from places.md, then correct the README to its output.

## Core theory

DRY: 'Every piece of knowledge must have a single, unambiguous, authoritative representation within a system', explicitly including documentation. Derived values should be computed from the source fields rather than hand-maintained to avoid 'source-of-truth drift caused by manually synchronizing derived values in multiple places'. A stored derived value is acceptable only with 'one visible authoritative recomputation path and consistency guardrails'.

## Applicability

places.md is the system of record for verdicts (README:373). The derived copies are: the summary table at README.md:393-403; each section's 'I read N places … Tally: better b, same s, worse w' line (e.g. README:409, 442, 471, 511); and the category breakdowns (e.g. '9 F, 52 T/MT, 14 sampled: 75 in all' at README:511). There is already one guardrail script, docs/evidence/sentence-cues/measure/check_readme.py, but it covers only the rendered-line tables. Extending that same script (no new file) to parse places.md sections, count '| better |', '| same |' and '| worse |' rows per '### <key>' heading, and assert the README summary rows, the All 8 row and each Tally/'N in all' line gives the required recomputation path. Then correct README to what the script reports: WjhDDeZ7DvM 76/45/22/9 and All 8 579/356/169/54.

## Hypothesis

The verdict tallies in README.md are a hand-maintained derived copy of places.md with no recomputation path and no check. Every round that edits places.md rows (verdicts, row splits) can silently stale them. The defect is the missing derivation and check, not one wrong number.

## Counterevidence

- places.md might be the side that is wrong: the 76th WjhDDeZ7DvM row could be an unintended split, or a duplicate of one place. All 76 timestamps are distinct and each row's new Korean is a different sentence, so this looks unlikely, but the row that changed has not been identified from history.
- The other seven videos' tallies match places.md exactly (verified by counting verdicts per section: 38/17/16/5, 23/11/8/4, 33/15/11/7, 263/204/44/15, 29/13/11/5, 60/26/29/5, 57/25/28/4). So the drift is a single slip, and one could argue a one-line number fix is enough.
- The category breakdown (9 F + 52 T/MT + 14 sampled = 75) cannot be machine-checked from places.md, because rows carry no category tag. A tally check alone leaves that sentence able to drift.

## Alternatives

- Hand-edit the four numbers only (README 398, 403, 511 and the 'in all' count). This is the smallest diff, but it is exactly what earlier rounds did. It leaves no guardrail, and the next row edit re-breaks it.
- Delete the README tallies and point to places.md. That removes the copy entirely, but the spec/review asks for a per-video summary, and the README sections give context that places.md lacks.
- Generate the README tables from places.md. This is heavier than a check, adds a build step for a doc, and is not needed when a check in the existing script suffices.

## Experiment

Extend check_readme.py to count verdicts per places.md section and compare them with the README summary rows, the All 8 row and each section's Tally/'N in all' line. Run it on the current tree with `python -I -X utf8 docs/evidence/sentence-cues/measure/check_readme.py`. It must report exactly the WjhDDeZ7DvM summary row, the All 8 row and the WjhDDeZ7DvM Tally/count line as differing, and nothing for the other seven videos. Then correct the README and rerun: differences 0. As a negative control, flip one verdict in a scratch copy of places.md, or temporarily in the tree and then revert; the check must report a difference. Separately, read the WjhDDeZ7DvM rows against the 9 F / 52 T-MT / 14 sampled breakdown to decide which category the 76th place belongs to, rather than bumping one number.

## Result

pending — the fix records what it observed here, with the tests and the commit.

## Unresolved limits

The specific commit and row that took WjhDDeZ7DvM from 75 to 76 is not identified (no git access here). That the breakdown sentence needs 76 = 9+52+15 or 9+53+14 is an inference that needs a reading of the rows. The grouped README per-video tables (e.g. '14:18–14:42') cannot be checked row-for-row by a tally check; only the totals can.
