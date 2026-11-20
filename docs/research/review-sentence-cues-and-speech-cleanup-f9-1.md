# Review research — F9, cycle 1

Draft research before the fix of review round 7 of PR #22, written 2026-11-20. It is not a verified fact or a shared rule: the review reads it with the fix.

## Problem

F8 again (inferred: the reviewer's text for this round was not in my context). The README's reading evidence has to match the per-place verdicts in places.md, which the README names as the authority (README.md:373). The round-6 fix made check_readme.py recompute the totals, tallies and place counts. It left the per-video breakdown sentences hand-typed. Two of them still disagree with the place count they explain. vSCYL8ySh9g (README.md:697-699) lists 5 filler + 7 misheard-term + 16 spread places = 28, against 29 places and a 13/11/5 tally (README.md:701, 410). _MEqhFwnp8o (README.md:579) lists 239 filler or repeat + 12 misheard-term + 16 spread = 267, against 'I read 263 places'. The summary evidence still contradicts the per-place record it summarises.

Trigger: recurrence, rounds 1, 2, 3, 7.

- [P1] [P1] docs/evidence/sentence-cues/receipts.md:3 — required runtime evidence is not bound to the reviewed head / when approving `cb2d047` / the enforced evidence obligations remain unproved

## Failed fixes

- Round 1 added places.md as the per-place record. That created a second copy of the counts, with nothing tying the README to it.
- Round 2 re-read places and split the vSCYL8ySh9g 9:44 row into two places (README.md:390-391). The tally and the summary row were updated by hand, but the breakdown list at README.md:697-699 was not. It still sums to 28, so the split left a stale copy.
- Round 4 rewrote the WjhDDeZ7DvM reasons without recounting the hand-typed totals. That is the drift cycle 1 found.
- Round 6 (cycle 1, 49878bf) extended check_readme.py to count the verdicts per places.md section. It checks the summary rows, the All 8 row, each Tally line, the first 'N in all' or 'I read N places' number, and the Regressions listing. The research page itself named the breakdown sentences as something the check cannot see, because places.md rows carry no category tag. So the check passes while the vSCYL8ySh9g and _MEqhFwnp8o breakdowns still disagree. The guardrail covered the totals, not the parts of a total.

## Sources

### S1 — Inclusion–exclusion principle — Wikipedia

- URL: https://en.wikipedia.org/wiki/Inclusion%E2%80%93exclusion_principle
- Retrieved: 2026-11-20
- Locator: Lead, two-set case
- Fragment: It 'generalizes the familiar method of obtaining the number of elements in the union of two finite sets', |A∪B| = |A| + |B| - |A∩B|, 'where A and B are two finite sets'. The sum of the two sizes 'may be too large since some elements may be counted twice'. Cardinality 'may be considered as the number of elements of the set, if the set is finite.'
- Applicability: The filler and misheard-term categories are finite sets of places that overlap: 4 _MEqhFwnp8o places carry both F and T. So the plain sum 239 + 12 + 16 = 267 overcounts the 263 places read.
- Counterevidence: It applies only where the categories overlap. Disjoint breakdowns (six of the eight videos) are plain sums.
- Validation: The extended check counts the intersection per video, and the README sentence states it, so stated parts minus overlap equal the place count.

### S2 — Schema as the contract — python-party concepts

- URL: https://deepbluecltd.github.io/python-party/concepts/schema-as-contract/
- Retrieved: 2026-11-20
- Locator: 'drift becomes structurally impossible' and its caveats
- Fragment: 'If there is exactly one source (the XSD) and everything else is generated from it', then 'drift becomes structurally impossible — you change the schema and re-generate, or you don't change the format at all.' Caveat: 'field-level type strength is only as rich as the XSD declares.' Drift is prevented in CI by regenerating the outputs and failing on any difference.
- Applicability: places.md is the one source, but it does not declare the selection category. Any count by category in the README therefore cannot be derived from it and stays hand-maintained. The CI-style guard is check_readme.py.
- Counterevidence: It is written for code generated from a schema, not for prose documentation, so it is applied by analogy. The README is checked against places.md, not generated from it.
- Validation: After the category column is added, check_readme.py derives every breakdown number from places.md and fails on a planted change.

### S3 — README.md — Reading old and new Korean (done item 15)

- URL: docs/evidence/sentence-cues/README.md
- Retrieved: 2026-11-20
- Locator: lines 373, 378-381, 390-391, 393-413, 579, 695-701
- Fragment: 373: 'The old and new Korean of every place, with its verdict, are in places.md'. 390-391: 'One vSCYL8ySh9g row at 9:44 covered two sentences and is now two places.' 410: '| `vSCYL8ySh9g` | 29 | 13 | 11 | 5 |'. 697-699: '- every old filler place (5); - every place marked as a misheard term (7) ... - the old cues that started or ended mid-sentence, spread evenly (16).' 579: 'I read 263 places: every old filler or repeat place (239), every misheard-term place (12, marks F/T), and 16 spread evenly.'
- Applicability: These are the derived copies. The vSCYL8ySh9g parts sum to 28 against 29 places, and the _MEqhFwnp8o parts sum to 267 against 263.
- Counterevidence: '(12, marks F/T)' may have been meant to signal overlap. The other six breakdowns add up.
- Validation: The extended check reports these two sections before the correction and nothing after it.

### S4 — places.md — Sentence cues: every place read, old and new Korean

- URL: docs/evidence/sentence-cues/places.md
- Retrieved: 2026-11-20
- Locator: header lines 3-6 and the table header '| At | Old Korean | New Korean | Verdict | Why |'; vSCYL8ySh9g section from line 466, two '9:44' rows at lines 472-473
- Fragment: Each row is one new sentence with columns At, Old Korean, New Korean, Verdict and Why. There is no column for why the place was selected (filler, misheard term, sampled). The vSCYL8ySh9g section holds two rows at 9:44, both 'same'.
- Applicability: This is the record the README summarises. Because it has no category field, no script can derive the breakdowns from it.
- Counterevidence: The category could be recovered from the uncommitted reading files instead of being added to places.md.
- Validation: Once places.md has a category column, the check counts it.

### S5 — check_readme.py — the README guardrail

- URL: docs/evidence/sentence-cues/measure/check_readme.py
- Retrieved: 2026-11-20
- Locator: lines 56-106
- Fragment: It counts verdicts per '### ' section of places.md and checks the summary row, 'Tally: better b, same s, worse w', and the place count: re.findall(r'([\d,]+) in all', text) or re.findall(r'I read ([\d,]+) places', text), first match only. It also checks that every worse place is listed under Regressions within 10 s. Nothing reads the per-category numbers.
- Applicability: This is why 'differences 0' at round 6 could coexist with the two inconsistent breakdowns, and it is where the per-category check belongs.
- Counterevidence: none
- Validation: After the extension it reports a planted tag change in a scratch places.md.

### S6 — Review research — F8, cycle 1

- URL: docs/research/review-sentence-cues-and-speech-cleanup-f8-1.md
- Retrieved: 2026-11-20
- Locator: Counterevidence bullet 3; Result item 5; Unresolved limits
- Fragment: 'The category breakdown (9 F + 52 T/MT + 14 sampled = 75) cannot be machine-checked from places.md, because rows carry no category tag. A tally check alone leaves that sentence able to drift.' Result 5: 'What the check cannot see: the F/T/sampled breakdown sentences, because rows carry no category tag'.
- Applicability: The previous cycle predicted this gap and left it open. This cycle closes it.
- Counterevidence: Cycle 1 fixed the WjhDDeZ7DvM breakdown by reading the rows, which shows the hand route can work once.
- Validation: The extended check covers what cycle 1 listed as unseen.

## Claims

- C1 (source; S3): README names places.md as the record of every place and verdict, and states per-video breakdowns of how the places were chosen.
- C2 (source; S3, S4): The vSCYL8ySh9g breakdown sums to 28 while the video has 29 places, after the round-2 split of the 9:44 row into two rows.
- C3 (interpretation; S1, S3): When categories overlap, adding their sizes overcounts the union by the size of the intersection. The _MEqhFwnp8o breakdown sums to 267, 4 more than its 263 places, which fits 4 places counted as both filler and misheard term.
- C4 (interpretation; S2, S4): A single source prevents drift only for what it declares. places.md has no selection-category field, so the README breakdowns cannot be derived from it.
- C5 (source; S5, S6, S2): check_readme.py checks totals, tallies, the first place count and the Regressions listing, but no breakdown numbers. Cycle 1 recorded this as a known gap.
- C6 (interpretation; S1, S2, S5, S6): The finding keeps coming back because each fix guarded one more derived copy while leaving another hand-typed. The fix that should hold is to add the selection category to places.md, check every breakdown, including overlaps, against it in check_readme.py, and correct the vSCYL8ySh9g and _MEqhFwnp8o sentences.

## Core theory

Two sources. The two-set inclusion-exclusion principle: for finite sets, |A∪B| = |A| + |B| - |A∩B|; the plain sum 'may be too large since some elements may be counted twice'. Single-source generation: 'If there is exactly one source ... and everything else is generated from it', then 'drift becomes structurally impossible'. That holds only if the source is as rich as what is generated from it ('only as rich as the XSD declares'), and CI regenerates and fails on any difference.

## Applicability

places.md is the one source for verdicts, but it is less rich than the README: it has At, Old, New, Verdict and Why, and no selection category. The README states category counts that cannot be derived from it. That is the 'only as rich as the schema declares' limit, and the reason the round-6 check could not reach the breakdowns. The selection tags already exist in the uncommitted reading files ([F], [T], [MT], [FM], [FMT] headers). Where: add a category column to places.md (or a tag in each At cell), filled from those tags when places.md is written. Extend docs/evidence/sentence-cues/measure/check_readme.py to count F, T and untagged places per section, including their overlap. Rewrite the breakdown sentences to state the overlap, e.g. '12 misheard-term places, 4 of them also filler places'. Apply the counts at README.md:419, 447-450, 481, 521, 579, 697-699, 737-739 and 788-791. vSCYL8ySh9g needs its split 9:44 [MT] place counted as two places, so 5 + 8 + 16 = 29, or the sentence has to say one place became two.

## Hypothesis

Each per-video breakdown ('every filler place (n), every misheard-term place (m), k spread evenly') is still a hand-maintained derived copy. places.md does not record the attribute it is grouped by, so no recomputation path exists. It went stale in two ways. (1) A row split in round 2 changed a place count without changing its parts: vSCYL8ySh9g, 28 vs 29. (2) The parts overlap, but the sentence reads as a disjoint sum. In the selection file 4 _MEqhFwnp8o places are tagged both F and T (1:38, 1:47, 33:19, 33:25), so 239 + 12 - 4 + 16 = 263 is correct while the text implies 267. The defect is the missing category field in the record of verdicts plus prose that ignores overlap. No single wrong number explains it.

## Counterevidence

- _MEqhFwnp8o may not be wrong at all: its parenthesis '(12, marks F/T)' may have meant the overlap. If so, only the wording misleads, and the total is right by the tags (263 headers in the selection file: 239 with F, 12 with T, 4 with both).
- Six of the eight breakdowns sum exactly to their place counts (QDX 20+2+16=38, QWNxQIq0hMo 7+16=23, WJpeW9lCSUM 2+5+20+6=33, WjhDDeZ7DvM 9+52+15=76, wbc-oxFFV9c 10+34+16=60, zjkBMFhNj_g 18+19+16+4=57). So one could argue two one-line edits are enough.
- I do not have the reviewer's text for this round. They may have refused on something else, e.g. the grouped per-video tables, which cycle 1 also listed as unchecked, or the receipts naming the fix commit only as 'the one after d079478'.

## Alternatives

- Hand-edit the two sentences (vSCYL8ySh9g to 29 parts, _MEqhFwnp8o to state the 4-place overlap). This is the smallest diff, but it repeats what rounds 1, 2 and 4 did, and the next row split breaks it again.
- Drop the numbers from the breakdown sentences and keep only the checked totals. This removes the copy, but the counts are the evidence that 'every filler place' and 'every misheard-term place' were read (README.md:378-381), so the coverage claim would lose its proof.
- Generate the whole README reading section from places.md. That is heavier than a check and adds a build step for a document, when a category column plus the existing check script is enough.

## Experiment

First, without editing, count the tags per video from the uncommitted selection files (artifacts/sentence-cues/read/<key>-sel.txt '## n t [TAGS]' headers): F, T, both, and neither. Compare them with each README breakdown. Prediction: exactly vSCYL8ySh9g (28 stated vs 29 rows, because 9:44 [MT] became two rows) and _MEqhFwnp8o (a disjoint sum of 267 vs 263, with an overlap of 4) differ, and the other six match. Then add the category column to places.md and extend check_readme.py to compare per-category counts with the breakdown sentences. Run `python -I -X utf8 docs/evidence/sentence-cues/measure/check_readme.py`: it must report those two sections before the README is corrected and 'differences 0' after. Negative controls on a scratch copy of places.md, passed as the optional argument: drop one F tag, split one row, and add an F tag to a T row. Each must be reported.

## Result

pending — the fix records what it observed here, with the tests and the commit.

## Unresolved limits

I am inferring which finding the review refused on; this round's reviewer text was not available. The tag counts come from the uncommitted artifacts/sentence-cues/read files, which may not match what places.md was built from at every commit. Whether the WjhDDeZ7DvM 9 F and 52 T/MT overlap was not checked. No shell or git was available, so no script was run, and the round-2 cause of the vSCYL8ySh9g drift is read from README.md:390-391, not from a diff.
