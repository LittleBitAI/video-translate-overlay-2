# Review research — F5, cycle 1

Draft research before the fix of review round 4 of PR #22, written 2026-11-19. It is not a verified fact or a shared rule: the review reads it with the fix.

## Problem

When a word ending in `etc.`, an initialism (`U.S.`, `a.m.`) or an ellipsis is followed by a lower-case word, `ends_sentence` (src-tauri/src/sentences.rs:194-214) has to decide whether a sentence ends there. Since b0b123c the same function sets both the cue boundaries (`push`/`ends`) and the translation groups (`rebuilt_parts`, pipeline.rs:464). The invariant: one sentence is one group. Two complete statements must not share a group, and one statement must not be split. Round 4: `We started at 9 a.m. this morning.` is split because `this` is in `OPENERS` (sentences.rs:218). `this morning.` then goes to Gemini as its own request, with only `context_before`.

Trigger: recurrence, rounds 1, 2, 3, 4.

- [P1] [P1] src-tauri/src/sentences.rs:217 — The new clause-opener rule splits ordinary time expressions.

## Failed fixes

- Round 1 -> 1ff5752: a question or exclamation mark became a sentence end even before lower case. That fixed `Any questions? we're done.`, but it left the period still suppressed before lower case. The quotation case was asserted the wrong way (`He asked "why?" and left.` expected as two groups).
- Round 2 -> 640b10c: a plain period now ends a sentence before lower case, except after an abbreviation, and a quoted ?/! that carries on stays inside. That holds. But initialisms, `etc.` and ellipses kept the rule 'continue whenever the next word is lower case', and round 3 found the false merge `in the U.S. the software was written`.
- Round 3 -> 1dcd50f: a 12-word `OPENERS` list (`the, we, i, you, he, she, it, they, there, this, these, those`) forces an end after `etc.` or an initialism. Most of those words also continue a noun phrase or start a reduced relative clause after such a word (`9 a.m. this morning`, `a.m. the next day`, `the U.S. these days`, `the U.S. we know`). So the list turned a rare false merge into common false splits. Its test (sentences_tests.rs:192-196) builds `{mark} {opener} came next.` for every opener, so every case is a clause by construction. It tests the list against itself and cannot fail in the other direction.

## Sources

### S1 — sentences.rs — ends_sentence, OPENERS, rebuilt_parts

- URL: src-tauri/src/sentences.rs
- Retrieved: 2026-11-19
- Locator: lines 189-218, 335-358
- Fragment: "`etc.` and an initialism also end one before a lower-case word that opens a clause ([`OPENERS`]: `in the U.S. the software`), which never carries a noun phrase on." ... `!(trails && lower) || (abbreviated && OPENERS.contains(&opener.as_str()))` ... `const OPENERS: [&str; 12] = ["the", "we", "i", "you", "he", "she", "it", "they", "there", "this", "these", "those"];` and rebuilt words take `Word { start_ms: c.start_ms, end_ms: c.end_ms, .. }`
- Applicability: The doc comment's claim 'never carries a noun phrase on' is what round 4 refutes: `this`/`the`/`these` are determiners. One function decides both cues and translation groups.
- Counterevidence: The rules for ?, ! and the plain period (lines 199-208) are not involved and hold.
- Validation: After the fix, line 213 no longer references any word list, and the both-directions table passes.

### S2 — sentences_tests.rs — punctuation table and opener loop

- URL: src-tauri/src/sentences_tests.rs
- Retrieved: 2026-11-19
- Locator: lines 170-196
- Fragment: `for (mark, opener) in ["U.S.", "a.m.", "etc."]...OPENERS...{ let said = format!("It ended at the {mark} {opener} came next."); ... assert_eq!(sentences_of(&cues), [format!("It ended at the {mark}"), format!("{opener} came next.")]` and `spoken(0, 200, 50, said)`
- Applicability: Every generated case is a clause by construction, and every gap is 50 ms. So the test can only confirm the list and never probes a continuation or a pause.
- Counterevidence: Line 180 does test one continuation (`U.S. data`, `a.m. and`), but neither follow-up word is on the list.
- Validation: Replace the loop with the both-directions table from the experiment.

### S3 — NLTK Punkt tokenizer source (Kiss & Strunk 2006)

- URL: https://www.nltk.org/_modules/nltk/tokenize/punkt.html
- Retrieved: 2026-11-19
- Locator: _second_pass_annotation [4.1.1 Orthographic Heuristic]; _ortho_heuristic
- Fragment: "If the token is an abbreviation or an ellipsis, then decide whether we should *also* classify it as a sentbreak." ... "If the word is lower case, and either (a) we've seen it used with upper case, or (b) we've never seen it used sentence-initially with lower case, then it's not a sentence starter." ... "Otherwise, we're not sure. return 'unknown'"
- Applicability: An established splitter treats abbreviation + lower case as no break, and it calls the remaining case uncertain rather than settling it with a word list. That supports 'continue unless other evidence says otherwise'.
- Counterevidence: Punkt's ortho statistics are learned on edited text, where lower-case sentence starts are rare. Whisper sometimes writes whole windows in lower case (README:298), which weakens the orthographic signal.
- Validation: The corpus count (S5) shows that abbreviation + lower case is a continuation in this repo's real output.

### S4 — splitta (Dan Gillick), companion to 'Sentence Boundary Detection and the Problem with the U.S.' (NAACL 2009)

- URL: https://pypi.tcis.de/data/web/json/splitta
- Retrieved: 2026-11-19
- Locator: package description
- Fragment: "The periods are potentially ambiguous, signifying either the end of a sentence, an abbreviation, or both." "But many abbreviations are more ambiguous (U.S., U.N.), which complicates the decision." "In training, "U.S." appears 2029 times, and 90 of these are sentence boundaries." "Of these, 24 (41%) involve the word "U.S.""
- Applicability: In newswire, `U.S.` ends a sentence about 4% of the time, and it causes a large share of a strong system's remaining errors. This slot is ambiguous in the literature, not only in this code. Continuing is the base-rate default.
- Counterevidence: These are newswire counts with any next word, not lecture speech or lower-case next words specifically. The paper PDF itself (https://aclanthology.org/N09-2061/) could not be read as text.
- Validation: Calibrating on one re-transcribed video (experiment 3) gives this repo's own rate.

### S5 — Sentence cues evidence README

- URL: docs/evidence/sentence-cues/README.md
- Retrieved: 2026-11-19
- Locator: lines 192-196; 31-38
- Fragment: "Cues holding two sentences fell from 239 to 5. None of the 5 holds two real sentences: three are periods the speech model wrote before a lower-case word ...; two are abbreviations ("Dr.", "U.S.")."
- Applicability: In the eight real videos, the abbreviation cues that a no-break rule would keep together are continuations, so the round-3 false-merge case was not seen in real output.
- Counterevidence: This was counted before the round-3 OPENERS rule and over only eight videos.
- Validation: Recount after the fix: no new two-sentence cues at abbreviation slots.

### S6 — Shriberg, Stolcke, Hakkani-Tür, Tür (2000), Prosody-Based Automatic Segmentation of Speech into Sentences and Topics

- URL: https://ar5iv.labs.arxiv.org/html/cs/0006036
- Retrieved: 2026-11-19
- Locator: 3.1.1 Broadcast News; 3.2.1 Switchboard; 2.1.2.1 Pause features
- Fragment: "Longer pause durations at the boundary imply a higher probability of a sentence boundary at that location." Pause was the most-used feature (46%) on Broadcast News, but on Switchboard pause usage was 18% (15% for the pause before the previous word), with pre-boundary duration on top at 49%. "Most inter-word locations contained no pause, and were labeled as zero length." Raw durations outperformed speaker-normalized durations.
- Applicability: This supports using the pause as the deciding evidence in the one slot the text cannot decide. It also shows the pause is probabilistic and weaker in conversational speech.
- Counterevidence: The pauses there came from forced alignment, not Whisper token timestamps. Hesitation pauses inside a sentence are not addressed.
- Validation: Experiment 3 compares gap distributions at known sentence ends against abbreviation continuations.

### S7 — pipeline.rs — translation groups and context

- URL: src-tauri/src/pipeline.rs
- Retrieved: 2026-11-19
- Locator: lines 461-464, 489-495
- Fragment: "The sentences are a function of the saved cues alone" `let groups = sentences(&cues, &crate::sentences::rebuilt_parts(&cues));` ... "One request per sentence (or part)" ... "The sentence before, read-only; never anything said later (A6)."
- Applicability: A false split sends `this morning.` alone, with only the previous group as context. A false merge sends both sentences together. The two errors cost different amounts.
- Counterevidence: A false merge still yields one cue holding two sentences (the round-1 complaint).
- Validation: The pipeline test from F2 still passes, and the round-4 fixture yields one request.

## Claims

- C1 (source; S1): Since round 3, any `etc.` or initialism followed by one of twelve listed lower-case words ends a sentence, and the code comment asserts these words never continue a noun phrase.
- C2 (source; S2): The round-3 test builds only clause continuations with 50 ms gaps, so it could not detect false splits such as `a.m. this morning`.
- C3 (source; S3, S4, S5): An abbreviation followed by a lower-case word is a recognised ambiguous case in sentence boundary detection. Established practice and this repo's own data treat it as a continuation by default.
- C4 (interpretation; S1, S2, S4): No closed list of next words can decide the slot in both directions, because the listed words (`this`, `the`, `these`, `we`) also continue a noun phrase or start a reduced relative clause. Rounds 2-4 alternated between false merges and false splits for this reason.
- C5 (source; S6): Pause duration at the boundary is the strongest single prosodic cue for a sentence boundary in read and broadcast speech, and weaker in conversation.
- C6 (interpretation; S1, S6, S7): Deciding the slot by gap (end only if lower && abbreviated && gap >= T, otherwise continue) keeps writer and rebuild consistent, because the gap survives at cue boundaries and is non-positive inside a rebuilt cue. It fixes round 4 and leaves the round-3 no-pause case as a documented limit for the person to decide.

## Core theory

Sentence boundary detection treats the period after an abbreviation as its hardest case, because it can mean an abbreviation, a sentence end, or both (Gillick, splitta). For an abbreviation, Punkt (Kiss & Strunk 2006, as implemented in NLTK) uses orthographic evidence: a following lower-case word that has been seen capitalized, or never seen lower case at a sentence start, is not a sentence starter, so there is no break. Lexical evidence alone leaves the remaining cases unresolved. In speech, Shriberg et al. (2000) found that pause duration at the boundary was the most-used feature for sentence segmentation in broadcast news. 'Longer pause durations at the boundary imply a higher probability of a sentence boundary', but pause matters less in conversational speech.

## Applicability

sentences.rs:213 is the only place the slot is decided. `abbreviated && OPENERS.contains(..)` should become `abbreviated && lower && gap >= T`, with `gap = next.start_ms - w.end_ms` computed inside `ends_sentence` from its two existing arguments. `ends` (line 76) and `bare` (line 170) call it with real word times. `rebuilt_parts` (line 347) gives every word in a cue that cue's start and end, so inside a cue the gap is negative and the slot never ends, which is what the writer did if it did not cut there. At a cue boundary the stored gap equals the word gap: `sentence_cues` only clamps a start to `prev_end` when words overlap, and then the gap is not positive anyway. So writer and rebuild make the same decision, which F2 requires. The translation stage (pipeline.rs:489-495) sends each group separately, with only the previous group as read-only context. A false split therefore loses the following words that complete the phrase (the round-4 harm). A false merge only puts two sentences into one request.

## Hypothesis

The slot 'etc./initialism/ellipsis + lower-case word' cannot be decided from the identity of the next word. Every closed word list has counterexamples in both directions, so each round moves the error from merge to split or back. The rule that holds stops reading the next word's identity in this slot. It ends the sentence there only on evidence the text does not carry: a pause at the boundary (next.start_ms - w.end_ms at or above a threshold). Otherwise the sentence continues, as Punkt does, which matches the repo's own corpus. Delete `OPENERS`. Keep the round-2 rules for ?, ! and the plain period. Writing and rebuild stay consistent, because the gap survives at every cue boundary and is zero or negative inside a rebuilt cue.

## Counterevidence

- Round 3's sentence `The hardware was built in the U.S. the software was written in Canada.` with no pause (test helper `spoken(0, 200, 50, ..)`, 50 ms gaps) would merge again. A reviewer may raise it again unless the limit is recorded as a decision.
- whisper.cpp token timestamps (whisper.rs:232, `set_token_timestamps(true)`) are approximate, and speakers can run sentences together without a pause or pause in the middle of a phrase (`at 9 a.m. ... this morning`). The pause is evidence, not proof, and the threshold T has not been measured on this data.
- Shriberg et al. found pause less dominant in conversational speech (18% feature usage on Switchboard against 46% on Broadcast News). Lectures fall between the two.

## Alternatives

- Drop `OPENERS` and always continue in this slot (pure Punkt behaviour). It is the smallest diff and matches the corpus (README:192-196: the two remaining abbreviation cues, `Dr.` and `U.S.`, are continuations), but it reopens round 3 with no evidence-based answer. It is the fallback if the pause turns out to be unusable.
- Narrow `OPENERS` to subject pronouns (`we, i, you, he, she, they`). Not first: these words still start reduced relative clauses (`the U.S. we know is`), so this is the same pattern that failed in rounds 2-4.
- Look at the word after the opener for a finite verb (a part-of-speech check). Not first: it needs a tagger or a verb list, which means a new dependency or another closed list, for a case that is rare in this input.
- Fix it only in translation by sending `context_after`. Not first: rule A6 (pipeline.rs:492) forbids anything said later in the request, and it would not fix the cue split.

## Experiment

1) Table test in both directions over the same marks (`U.S.`, `a.m.`, `etc.`, `so...`). (a) Continuations with 50 ms gaps must stay one sentence: `at 9 a.m. this morning.`, `at 9 a.m. the next day.`, `in the U.S. these days.`, `the U.S. we know is big.`, `GGUFs etc. the same way.`. (b) A clause after a gap of T or more must split: `built in the U.S.` [pause] `the software was written in Canada.`. (c) The same clause with a 50 ms gap must stay one sentence, a documented limit. The hypothesis is refuted if (a) and (b) cannot both pass for any T. 2) Consistency: run the F2 round-trip property test (push -> saved cues -> `rebuilt_parts`) on sequences that include these slots with gaps around T. The rebuilt grouping must equal the grouping the writer cut. 3) Calibration: on one re-transcribed video, log the gap at every period-ended word that is followed by an upper-case sentence start, and at every abbreviation followed by lower case. Set T below most sentence-end gaps and above most continuation gaps. If the two distributions overlap heavily, take the fallback alternative (always continue).

## Result

pending — the fix records what it observed here, with the tests and the commit.

## Unresolved limits

This session had no shell, so no test was run and T is not measured. I don't know how often whisper output puts a real pause at these slots. Whether to accept the round-3 no-pause merge as a known limit is the person's decision and should be recorded before the fix. The Gillick paper's PDF could not be read as text, so its figures come from the same author's splitta package description.
