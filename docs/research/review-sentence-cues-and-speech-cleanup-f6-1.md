# Review research — F6, cycle 1

Draft research before the fix of review round 4 of PR #22, written 2026-11-19. It is not a verified fact or a shared rule: the review reads it with the fix.

## Problem

F6: one grammatical spoken sentence must go to Gemini in one request with all its parts (A6). Only a stretch with no punctuation at all may be ended at the 210-character run-on limit (STRETCH_MAX). The invariant breaks whenever ends() decides that a sentence is 'bare' when it is not. bare() (src-tauri/src/sentences.rs:164-178) counts only ',', ';' and ':' as punctuation and treats reaching the sentence's own end mark as proof of bareness. A 250-character sentence whose only punctuation is its final period is therefore split. bare() also stops looking after LOOK = 420 characters, so a sentence whose first comma comes after 420 characters is split as well. The split happens in rebuilt_parts, which now alone decides the translation groups (pipeline.rs:464).

Trigger: recurrence, rounds 1, 2, 3, 4.

- [P1] [P1] src-tauri/src/sentences.rs:177 — The lookahead still splits correctly punctuated sentences at the 210-character cap.

## Failed fixes

- Round 1 (3f8c8ed): the finding was disputed and the 210-character cap was kept. The reviewer then showed with the Dory 24:23 example that a cut at a comma turned part of a question into a statement.
- Round 2 fix (1ff5752 -> 640b10c): a stretch already holding a comma was never ended early. The comma test looked only at words up to the current one, so a sentence whose first comma comes after 210 characters was still classified as bare before the loop reached the comma.
- Round 3 fix (640b10c -> 1dcd50f): bare() searched the words still to come, up to LOOK = 2*STRETCH_MAX = 420 characters. This moved the boundary instead of removing it: a first comma at 421+ characters is still bare. It also returned Some(true) on reaching the sentence end mark, so a sentence punctuated only by its final period counts as bare. All three rounds decided a property of the whole sentence (is there punctuation before it ends?) from a bounded prefix of it, so each fixture just past the latest window reopened the finding.
- Round 4 (1dcd50f -> ff0c814, the F2 fix b0b123c): grouping moved to rebuilt_parts over the complete saved cues with last=true, which makes the full sentence visible. bare()'s LOOK bound and its comma-only test were left in place, so F6 was not addressed (correction: none yet).

## Sources

### S1 — sentences.rs: ends(), bare(), rebuilt_parts

- URL: src-tauri/src/sentences.rs
- Retrieved: 2026-11-19
- Locator: lines 31-36, 76-82, 150-178, 335-358
- Fragment: "A sentence with a comma in it, wherever the comma comes (searched up to [`LOOK`] characters), is never ended early: it goes whole ... however long." and `fn punctuated(ws) -> ws.iter().any(|w| w.text.trim_end().ends_with([',', ';', ':']))`; in bare(): `let ended = ... ends_sentence(&ws[j], n) || gap >= SENTENCE_GAP_MS ...; if ended || len(&ws[from..=j]) >= LOOK { return Some(true); }`; `const LOOK: usize = 2 * STRETCH_MAX;`
- Applicability: The classifier that decides every split past 210 characters, in both push and rebuilt_parts.
- Counterevidence: The bare 60-word fixture shows the fallback is needed for genuinely unpunctuated speech.
- Validation: Fixtures (a) and (b) fail on HEAD and pass after the fix; fixture (c) still splits.

### S2 — backend.md A6, revised with sentence cues

- URL: docs/design/backend.md
- Retrieved: 2026-11-19
- Locator: line 56, 'A sentence with a comma in it goes whole'
- Fragment: "A sentence with a comma in it goes whole to its sentence end however long it runs (decided by the person on 2026-11-19 ...); only a stretch with no punctuation at all is ended at about three cue lengths (210 English characters). The punctuation is looked for in the words still to come too (up to 420 characters)" ... "A reply with a part missing, extra or repeated is not saved, and the sentence is asked again whole; it is never sent one part at a time. If three passes in a row save nothing, the job stops with `translate_failed`"
- Applicability: The spec both promises 'however long' and bounds the search at 420; 'no punctuation at all' includes the end mark. A sentence too long for one reply can never be saved.
- Counterevidence: One could read 'punctuation' as inner punctuation only, which would make the period-only case spec-conformant, but 'at all' argues against that reading.
- Validation: After the fix, A6 names one ceiling and no longer says both 'however long' and 'up to 420'.

### S3 — sentences_tests.rs: comma and bare fixtures

- URL: src-tauri/src/sentences_tests.rs
- Retrieved: 2026-11-19
- Locator: lines 139-167, a_sentence_with_commas_goes_whole_however_long_and_only_a_bare_stretch_ends_at_the_limit
- Fragment: `let opening = (0..40).map(|i| format!("step{i}"))...; let late = format!("{opening}, and only then does the sentence end.");` with cases comma-chained, one comma, 'first comma after the limit' and 'no punctuation, no pause' -> 2
- Applicability: The opening is about 270 characters, inside LOOK, and no fixture has a sentence whose only punctuation is its end mark. The suite cannot see either reported case.
- Counterevidence: none
- Validation: Add fixtures (a), (b) and (d) here.

### S4 — gemini.rs reply cap and pipeline.rs grouping point

- URL: src-tauri/src/gemini.rs
- Retrieved: 2026-11-19
- Locator: gemini.rs:22 and :96; pipeline.rs:461-464
- Fragment: `pub const MAX_OUTPUT_TOKENS: i64 = 4096;` ... `"maxOutputTokens": MAX_OUTPUT_TOKENS`; pipeline.rs: "The sentences are a function of the saved cues alone ..." `let groups = sentences(&cues, &crate::sentences::rebuilt_parts(&cues));`
- Applicability: Grouping happens after the full transcript exists, so full lookahead is free there. The reply cap is the one hard bound on sentence size.
- Counterevidence: The real Korean token size of the largest sentences is unmeasured, so the cap may be far from any real sentence.
- Validation: Recount Korean output tokens of the largest replies on the eight versions.

### S5 — Sentence cues evidence README

- URL: docs/evidence/sentence-cues/README.md
- Retrieved: 2026-11-19
- Locator: lines 39-44, 85-95, 190, 385
- Fragment: "Only a stretch with no punctuation is still ended at 210 characters." ... "the longest holds 18 parts, 763 characters and 40.9 s (`QDX-1M5Nj7s`, from 45:11), and 5 run over 32 s." ... "In the new transcript, whisper left long stretches without punctuation, spread across the whole hour. 86 sentences open in lower case."
- Applicability: Real data shows both the current request sizes and the size of the unpunctuated population that the new definition would move from the 210 fallback to the ceiling.
- Counterevidence: It is evidence for the risk that requests grow large, so it argues for a ceiling, not for 'however long'.
- Validation: Recount the table under the new definition before choosing the ceiling.

### S6 — Gemini finish_reason: why generation stopped

- URL: https://theneuralbase.com/gemini-api/learn/beginner/finish-reason-why-generation-stopped/
- Retrieved: 2026-11-19
- Locator: response table and field guide on MAX_TOKENS
- Fragment: text "may be incomplete if finish_reason is MAX_TOKENS"; "If finish_reason is MAX_TOKENS, this is truncated mid-thought."; "you pay for MAX_TOKENS outputs even though they're truncated and likely unusable." (A third-party guide, not Google documentation; it does not say whether structured JSON stays valid.)
- Applicability: A sentence whose Korean exceeds 4,096 tokens gets a truncated, unparseable reply on every try, so unbounded 'however long' grouping can fail a job.
- Counterevidence: Third-party page; Google's own docs were not fetched.
- Validation: A test with a fake Gemini that truncates at the cap, plus the token recount.

### S7 — Semantic Segmentation with Bidirectional Language Models Improves Long-form ASR (Huang, Zhang, Kumar, Chang, Sainath, Interspeech 2023)

- URL: https://arxiv.org/abs/2305.18419
- Retrieved: 2026-11-19
- Locator: abstract
- Fragment: Segmentation aims at "separating semantically complete sentences within the utterance", which "prevents the ASR decoder from needlessly processing faraway context" while not "missing relevant context within the current sentence"; the segmenter is distilled from "a bidirectional teacher language model (LM) trained on written, punctuated text" and evaluated "on a streaming ASR pipeline". (The abstract does not discuss lookahead bounds explicitly.)
- Applicability: Supports the distinction used here: a full-context segmenter can find complete sentences, while a streaming one has to approximate. Translation grouping here runs with full context, so it does not need the streaming approximation (LOOK).
- Counterevidence: It is about ASR decoding segments, not translation requests, and does not state the impossibility argument; that argument is my inference.
- Validation: Fixture (b) passes only once the classifier sees the whole sentence.

### S8 — Web search: streaming ASR sentence segmentation punctuation lookahead latency

- URL: https://arxiv.org/pdf/2606.05179
- Retrieved: 2026-11-19
- Locator: search result summary (paper not fetched)
- Fragment: Search summary: streaming punctuation work "compares punctuation insertion hypotheses against a no-insertion baseline under a bounded K-subword-token lookahead"; other cited work reports that "one future word can be insufficient" and that larger context improves end-of-segment accuracy "but so does the decoder latency". (Snippet-level only; full results not verified.)
- Applicability: Bounded lookahead is a latency trade-off in streaming, accepted knowing it misclassifies some inputs. Here translation has no latency need for it.
- Counterevidence: Not fetched in full; the claims rest on search snippets.
- Validation: none beyond fixture (b)

## Claims

- C1 (source; S1): bare() returns Some(true) when the stretch reaches its own sentence end mark with no ',', ';' or ':', so a sentence punctuated only by its final period is split at 210 characters.
- C2 (source; S1, S2): bare() stops at LOOK = 420 characters, so a sentence whose first comma comes later is split. The spec text both promises 'however long' and states 'up to 420'.
- C3 (interpretation; S1, S2): Every fix from round 2 to round 4 decided a whole-sentence property from a bounded prefix, so a fixture just past each new window reopened the finding.
- C4 (source; S3): No existing fixture places a first comma beyond LOOK or has a sentence whose only punctuation is its end mark.
- C5 (interpretation; S2, S4, S6): A literal 'however long' cannot hold: a sentence whose Korean exceeds the 4,096-token reply cap is refused on every pass and ends the job with translate_failed, so one documented ceiling derived from that cap is needed.
- C6 (interpretation; S4, S7, S8): Translation grouping is computed after transcription on all saved cues, so it can decide with full context. LOOK is a streaming approximation with no reason to apply there.
- C7 (interpretation; S5): Counting the end mark as punctuation moves many of whisper's long weakly punctuated stretches from the 210 fallback to the ceiling, so request sizes must be recounted before the ceiling is chosen.

## Core theory

An online decision about a property that depends on an unbounded suffix of the input cannot be made correctly with any fixed lookahead: for every window K there is an input whose deciding token sits at K+1. Streaming segmenters accept that trade-off because waiting for right context costs latency, while segmenters with full (bidirectional) context do not have to. Where the whole input is available, the decision should be made on the whole input, and the only bound left should be a real resource limit, here the model's maximum output per reply, beyond which a reply is truncated.

## Applicability

sentences.rs:76-82 sends every word past the cap to bare(ws, from, last). bare (164-178) returns Some(true) when `ended` (a mark or a 3-second pause) or len >= LOOK, and Some(false) only for ',', ';' or ':'. rebuilt_parts (335-358) calls ends(&words, true) on all saved cues of a seq-contiguous run, and pipeline.rs:464 uses only that grouping, after the whole transcript exists. The streaming writer push (52-62, via stitch.rs:79) affects only where cues are cut, not which cues are translated together, so an end it forces early is merely a part boundary that the rebuild can join. With one ceiling C used in both places, push holds at most C characters (None until a mark, a comma, a 3-second pause or C), so the tail stays bounded. A reply that cannot fit 4,096 output tokens comes back truncated, is refused as missing parts, and is asked again whole. After three passes with nothing saved the job stops with translate_failed (A6), so some ceiling is required, and its value should come from that cap.

## Hypothesis

The recurrence has two causes in bare(). (1) It tests the wrong property: the spec's 'no punctuation at all' includes the sentence's own end mark, but the code counts only ',', ';' and ':', and the `ended` branch returns true (bare) when a mark is reached. (2) Its lookahead is bounded by LOOK, a classifier window rather than a limit on the sentence, so a fixture just past it always exists. A6 and the comment at sentences.rs:31-36 both say 'however long' while also saying 'up to 420', so the spec contradicts itself and the reviewer enforces the 'however long' half. Translation grouping now runs only after transcription, on all cues (pipeline.rs:464, last=true), so no streaming constraint justifies LOOK there. The fix that holds: (a) treat a reached sentence end mark as punctuation, so bare means 'reaches a 3-second pause or the end of the run with no mark and no comma'; (b) replace LOOK with a single sentence ceiling, the same constant in push and rebuilt_parts. The ceiling should be derived from the one real limit, the 4,096-token reply cap (gemini.rs:22). A6 and the code comment should name it in place of both 'however long' and 'up to 420', so the documented boundary and the code are the same thing. Above the ceiling a sentence is cut as today. The number is the person's decision.

## Counterevidence

- Whisper leaves long weakly punctuated stretches; QDX has 86 forced ends in unpunctuated stretches (docs/evidence/sentence-cues/README.md:190, 385). If many of them end at a period much later, counting the end mark as punctuation turns them into very long single requests. Most would then hit only the new ceiling rather than the 210 fallback, and the Korean of a run-on in one request might read worse or exceed the reply cap.
- The largest request seen so far is 763 characters / 18 parts / 40.9 s and succeeded (README:87-95), so the real distance to the 4,096-token cap is unmeasured. The ceiling value is a guess until the eight saved versions are recounted.
- Any finite ceiling can again be exceeded by a constructed fixture. It holds only if A6 states the ceiling as the definition of the limit and drops 'however long', which is a spec change the person must approve.
- Removing LOOK without making bare() incremental makes it roughly cubic in a long stretch: ends() calls bare from the same `from` at every word past the cap, and each call re-measures len(&ws[from..=j]). An unpunctuated run of thousands of words in rebuilt_parts could stall translation start.

## Alternatives

- Remove LOOK only (unbounded comma search, comma-only definition): fixes the 421+ comma case, but the 250-character period-only sentence still splits, so round 5 would raise that half.
- Raise LOOK (e.g. to 1,000): the same pattern that failed in rounds 2-4; the next fixture sits at LOOK+1.
- No ceiling at all ('however long' literally): honors the current spec text, but a sentence whose Korean exceeds 4,096 output tokens is refused on every try and the job ends with translate_failed after three passes. That is worse than a split.
- Keep the 210 fallback and narrow the spec to 'a comma within 420 characters': the person decided on 2026-11-19 that comma sentences go whole, and the Dory 24:23 reading shows the harm of cuts, so narrowing the spec reverses a recorded decision.
- Split long sentences but send them in one request as read-only context for each other: A6 forbids content from later sentences in a request (only the sentence before goes along as context), so this changes A6 more than a ceiling does.

## Experiment

1) Unit tests in sentences_tests.rs, through sentences_of (rebuilt_parts) and spoken(...) with short gaps: (a) a 250-character sentence whose only punctuation is its final period, then a second sentence, gives 2 sentences, the first whole; (b) an opening of about 450 characters with no punctuation, then ', and only then does the sentence end.' gives 1 sentence; (c) the existing 60 bare words (and a variant ending at a 3,200 ms pause) still give 2 or more, so the fallback still fires; (d) a sentence longer than the chosen ceiling is cut, and no request exceeds it; (e) the same words fed to push in several windows (last=false, then true) give the same rebuilt sentences as one window. Run on current HEAD, (a) and (b) are expected to fail; with the fix all pass. 2) Performance guard: rebuilt_parts over 3,000 unpunctuated words finishes in well under a second. 3) Before choosing the ceiling, recount the eight saved versions on a read-only copy with the new definition and no ceiling: largest request (characters, parts, seconds) and the Korean output tokens of the largest replies against 4,096. Choose the ceiling with headroom, have the person approve it as options, then write it into A6 and the sentences.rs comment in place of 'however long' and 'up to 420'. 4) Full repository gate (gate_cmd in .wiki/adapter.toml), not a single cargo test.

## Result

pending — the fix records what it observed here, with the tests and the commit.

## Unresolved limits

Not executed: I can neither run tests nor run a shell, so the failure of fixtures (a) and (b) on HEAD is predicted from reading bare() and ends(), not observed. The Korean-token-per-English-character ratio for gemini-3.1-flash-lite, and therefore the safe ceiling, is unknown. Whether the 3-second-pause rule alone keeps whisper's unpunctuated stretches small is unmeasured. Whether the reviewer accepts a documented ceiling as satisfying 'all parts of a long sentence travel together' depends on the person approving the spec wording. Web sources on streaming lookahead are general ASR results, not about this code.
