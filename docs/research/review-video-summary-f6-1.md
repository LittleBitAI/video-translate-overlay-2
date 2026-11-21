# Review research — F6, cycle 1

Draft research before the fix of review round 4 of PR #23, written 2026-11-21. It is not a verified fact or a shared rule: the review reads it with the fix.

## Problem

For a lecture whose recognized language is known, summary::check must store topics and next_topics only when every phrase is written in the lecture's language (backend.md:186-190). Phrases in another language must be refused. Valid technical terms in Latin or Greek letters must still pass. At ea03bdf (same code as 485fab7), next_topics are only checked for which alphabet their letters belong to (summary.rs:140, in_scripts at summary.rs:93-96). For a Latin-alphabet language, any Latin phrase passes, so a French recommendation is stored for an English lecture. The test at summary.rs:404-406 expects that pass.

Trigger: recurrence, rounds 1, 2, 3, 4.

- [P1] [P1] docs/evidence/video-summary/report.json:3 — The generated receipt and focused output are missing from the round packet

## Failed fixes

- Round 1 (22e5639 -> 13e28b4): refused Hangul in topics and next_topics for a non-Korean lecture. This tests one alphabet, not a language, so kana, Cyrillic and other alphabets passed. The new test computed its expected values with the same Hangul check, so it could not catch the gap.
- Round 2 (13e28b4 -> 817c63b): added a per-language alphabet table (scripts/in_scripts, summary.rs:76-96), with Latin and later Greek allowed in every language. That closed the Japanese-on-English case. But the table gives all 33 Latin-alphabet languages no alphabet of their own (summary.rs:77-85), so it cannot tell them apart, and French still passed for English.
- Round 3 (817c63b -> 485fab7): added a word-overlap test (in_talk, summary.rs:110-114): each topic must share a word of three or more letters with the title, glossary or transcript. This closes same-alphabet translation for topics. But next_topics may name subjects the talk never mentions, so the overlap test was applied to topics only (summary.rs:143). next_topics stayed on the alphabet check. The test row was written to expect the French phrase to pass (summary.rs:404-406), pinning the reported bug as correct behaviour.
- Common cause across rounds: each fix answered the reviewer's latest example with a cheaper structural signal (alphabet, word overlap) and added a test row for that one example. The fix also followed backend.md:202-204, which explicitly limits next_topics to the alphabet check. That contradicts backend.md:189-190, which says next_topics are in the lecture's language. The reviewer reads the requirement, the fixer reads the carve-out, so every round reopened the same gap. Cycle 1 research (docs/research/review-video-summary-f3-1.md) found this, but no fix has been made since.

## Sources

### S1 — src-tauri/src/summary.rs at ea03bdf

- URL: src-tauri/src/summary.rs
- Retrieved: 2026-11-21
- Locator: lines 76-96 (scripts, in_scripts), 110-114 (in_talk), 140-145 (check), 404-406 (test rows)
- Fragment: l if LATIN.contains(&l) => &[], ... fn in_scripts(text: &str, language: &str) -> bool { let Some(own) = scripts(language) else { return true }; text.chars().filter(|c| c.is_alphabetic()).all(|c| { let s = script(c); matches!(s, Script::Latin | Script::Greek) || own.contains(&s) }) } ... if field == "topics" && language.is_some_and(|l| !kept.iter().all(|t| in_talk(t, l, &source))) ... // Next topics come after the talk, so they are not held to its words: a same-script one passes. (Some("en"), "next_topics", accented, true),
- Applicability: Shows that next_topics for Latin-alphabet languages pass any Latin phrase, and that the test expects a French phrase to pass for 'en'.
- Counterevidence: in_talk does refuse same-alphabet translation for topics; only next_topics stay open.
- Validation: After the fix, the row at line 406 expects refusal (or is removed under the spec fallback), and accent-free translation rows are added.

### S2 — docs/design/backend.md, Video summary (P19), output form

- URL: docs/design/backend.md
- Retrieved: 2026-11-21
- Locator: lines 186-207
- Fragment: `topics` (the lecture's language) ... `next_topics` (the lecture's language, as search phrases for the later recommendation step) ... For a language written with spaces between words (the Latin, Cyrillic and Greek ones), every topic must also share a word of three or more letters with the title, glossary or transcript, so a same-script translation (French words for an English talk) is refused. Next topics, which come after the talk, are held only to the scripts: requiring the talk's words would refuse correct replies the person pays for.
- Applicability: The spec both requires next_topics in the lecture's language and limits their check to the alphabet. The reviewer and the fixer each follow one half.
- Counterevidence: The carve-out's reason (cost of refusing correct paid replies) is legitimate and also argues against a strict detector.
- Validation: The fix changes this paragraph through spec-update, either describing the detector rule or stating the relaxed guarantee.

### S3 — lingua 1.8.0 (crate README on docs.rs)

- URL: https://docs.rs/crate/lingua/latest
- Retrieved: 2026-11-21
- Locator: accuracy report (German); minimum relative distance; low accuracy mode; language features; confidence values
- Fragment: German, high accuracy mode: 73.9% on 1,000 single words, 94.1% on 1,000 word pairs, 99.7% on sentences; the single-word test data is 'a list of single words with a minimum length of 5 characters'. None 'is the return value for cases where language detection is not reliably possible.' 'Be aware that the distance between the language probabilities is dependent on the length of the input text.' Low accuracy mode: 'detection accuracy for short texts consisting of less than 120 characters will drop significantly.' Default dependencies total 'approximately 300 MB'; languages can be chosen with default-features = false and features such as ["french", "italian", "spanish"]. compute_language_confidence_values returns values from 0.0 to 1.0; 'If the language is unambiguously identified by the rule engine, the value 1.0 will always be returned.'
- Applicability: The kind of tool that can separate same-alphabet languages. Its short-input accuracy sets how lenient the next_topics rule must be, and feature selection limits its build cost.
- Counterevidence: Figures are the project's own German benchmark. English/French per-language numbers are only in chart images. Single-word data excludes words under 5 characters.
- Validation: The fixture experiment measures false refusals and misses on this project's phrases with the selected languages.

### S4 — whatlang (crate README on docs.rs)

- URL: https://docs.rs/crate/whatlang/latest
- Retrieved: 2026-11-21
- Locator: features list; reliability; package size
- Fragment: 'Fast and lightweight language identification library for Rust.' 'Supports 70 languages.' Reliability 'is based on the following factors': the number of unique trigrams in the text and the gap between the top two detected languages. No statement on short-text accuracy. Crate source size 685.9 kB.
- Applicability: A much smaller alternative detector for the Android build. Its reliability flag predicts that short next_topics phrases will often be unreliable.
- Counterevidence: No published short-text accuracy. Lingua's author reports lingua beating whatlang on single words and pairs (an author benchmark).
- Validation: Run it in the same fixture experiment as S3 before choosing it.

### S5 — docs/evidence/video-summary/README.md, paid run

- URL: docs/evidence/video-summary/README.md
- Retrieved: 2026-11-21
- Locator: lines 74-86
- Fragment: - mechanistic interpretability - reinforcement learning from human feedback - retrieval augmented generation - scaling laws - adversarial machine learning ... Prose fields came back in Korean; topics and next topics stayed in English, the lecture's language.
- Applicability: Real should-pass next_topics phrases for the fixture. Also shows the failure has not been observed live.
- Counterevidence: A single run; it does not show Gemini never translates recommendations.
- Validation: All five phrases must pass under any new rule.

### S6 — docs/research/review-video-summary-f3-1.md (cycle 1 research)

- URL: docs/research/review-video-summary-f3-1.md
- Retrieved: 2026-11-21
- Locator: Failed fixes; Hypothesis; Result: 'pending'
- Fragment: Across all three rounds, each fix was fitted to the reviewer's latest example and added a test row for it. No fix tested the general case (a same-alphabet translation, with or without accents) ... Result: pending — the fix records what it observed here, with the tests and the commit.
- Applicability: Records the earlier analysis. Its Result is still pending, so the experiment has never been run.
- Counterevidence: It is a draft, not a verified finding.
- Validation: The next fix fills in its Result with the experiment's observed numbers.

## Claims

- C1 (source; S1): At ea03bdf, next_topics for a Latin-alphabet lecture language are checked only for Latin or Greek letters, and the test expects an accented French phrase to pass for 'en'.
- C2 (interpretation; S1, S2, S6): The finding recurs because the spec requires next_topics in the lecture's language yet limits their check to the alphabet. Each fix followed the carve-out and answered only the reviewer's literal example, and no experiment from cycle 1 has been run.
- C3 (interpretation; S3, S4): Only a statistical detector can separate same-alphabet languages in code, and its reliability on one- or two-word inputs is clearly lower than on sentences, so a strict refusal would refuse correct replies.
- C4 (interpretation; S2, S5): The carve-out's reason is the cost of refusing paid, correct replies. The only live evidence shows correct English next_topics, so tightening versus formally relaxing is a trade-off for the person.
- C5 (source; S4): Whatlang's reliability depends on unique trigram count and the top-two gap, and it is a 685.9 kB crate.

## Core theory

Language identification on short inputs is a statistical problem whose reliability grows with text length. Lingua reports, for its German benchmark in high-accuracy mode, 73.9% on single words (at least 5 characters), 94.1% on word pairs and 99.7% on sentences. It warns that the distance between language probabilities depends on input length, so a high minimum relative distance makes short inputs return no language ('language detection is not reliably possible'). Its low-accuracy mode drops significantly below 120 characters. Whatlang, a lighter trigram detector derived from Franc, bases its reliability flag on the number of unique trigrams and the gap between the top two languages, so short phrases are flagged unreliable by design. An alphabet test cannot separate languages that share an alphabet.

## Applicability

summary.rs:140 is the only gate on next_topics. For language codes in the LATIN table (summary.rs:77-78) in_scripts accepts any Latin phrase, so French, Spanish or German translations pass for 'en' with or without accents. A detector would apply at this point, for next_topics only, and only for languages written with spaces (the same set in_talk uses at summary.rs:111). Candidates would be limited to the talk's language plus the main Latin-alphabet languages. The rule would refuse only when another candidate's confidence clearly beats the talk's language, which keeps short borrowed terms passing. topics already have in_talk and need no change. The same change must flip or remove summary.rs:406 and add accent-free translation rows. backend.md:202-204 must be rewritten through spec-update, because the current text forbids the stricter rule.

## Hypothesis

The finding keeps coming back for two reasons. The spec contradicts itself about next_topics. And the only signals the code uses (alphabet and word overlap with the talk) carry no information about which of several same-alphabet languages a short phrase is in. A fix can hold only if it either adds a statistical language detector for next_topics in Latin, Cyrillic and Greek languages, lenient enough not to refuse borrowed technical English, or, through spec-update, drops the requirement that next_topics are verified to be in the lecture's language beyond the alphabet. Both need the backend.md:202-204 paragraph and the summary.rs:406 test row changed in the same change.

## Counterevidence

- The paid run (docs/evidence/video-summary/README.md:74-86) returned five English next_topics for an English lecture. The failure has never been seen live; it is hypothetical model behaviour.
- Lingua's own benchmark gives about 74% on single words (German). A hard refusal on a short phrase would sometimes refuse correct replies the person paid for, which is exactly why backend.md:203-204 left next_topics lenient.
- French and German technical talks commonly use English terms as-is (deep learning, transformer). A detector may label a correct next topic as English for a fr/de talk, or label a borrowed term as foreign for an en talk.
- Lingua's default dependencies are about 300 MB. Only feature-selecting a few languages keeps it small, and the app also ships an Android build, so cost may outweigh benefit.

## Alternatives

- Refuse accented Latin letters for 'en': closes the reviewer's literal example but not 'apprentissage profond' or 'aprendizaje por refuerzo'. It would be a fifth fix of the same failed kind.
- Per-language function-word lists (de, la, des, der, und, del): cheap, but misses one-word and noun-only phrases. Still a heuristic arms race.
- Require next_topics to share a word with the talk: rejected, because recommendations are meant to name subjects beyond the transcript (backend.md:202-204).
- A second Gemini call to verify the language: P19 is designed as one call and a second doubles the cost. That is the person's decision.
- Whatlang instead of lingua: much smaller (685.9 kB crate) and faster, but its page gives no short-text accuracy figures, and lingua's author reports lingua beats it on single words and pairs. Worth putting in the same experiment, not adopting untested.
- Spec-only change (next_topics guaranteed only to be in the right alphabet, stated in backend.md and the code comment, test rows kept): honest and cheap, but it relaxes a stated requirement. The person must approve it through spec-update. It is the fallback if the detector experiment fails.

## Experiment

Build a fixture of next_topics phrases. Should pass: the five English recommendations from the paid run (mechanistic interpretability; reinforcement learning from human feedback; retrieval augmented generation; scaling laws; adversarial machine learning), plus English technical terms marked as valid for fr/de talks. Should fail: 15 or more French, Spanish and German translations with and without accents (réseaux de neurones, apprentissage profond, aprendizaje por refuerzo, tiefes Lernen). Run lingua with default-features=false and only en/fr/de/es/it/pt/nl, using compute_language_confidence_values. Refuse a phrase only when another language's confidence exceeds the talk language's by a chosen margin. Run whatlang with the same allowed set. Adopt a detector only if zero should-pass phrases are refused and most should-fail phrases are caught, and only if the Android build size stays acceptable. Otherwise take the spec-change fallback to the person. In either case, change summary.rs:404-406 and backend.md:202-204 in the same commit as the rule.

## Result

Observed 2026-11-21, draft, not verified. The experiment is the same as `docs/research/review-video-summary-f3-1.md`. lingua with 7 languages refused 0 of 45 correct phrases at margin 0.4 and caught 28 of 36 same-alphabet translations (0.5: 0 refused, 26 caught). At 0.3 it still caught 32, but the closest correct phrase sat at +0.282. The binary grew to about 33 MB. whatlang refused 26 correct phrases. The person chose not to add a detector. The contradiction this page found, backend.md calling next_topics "the lecture's language" while the code checks only scripts, is resolved by wording. The spec's done item 3 and backend.md now say next_topics are asked in the talk's language and guaranteed only in its scripts. The test rows in `summary::tests::each_field_is_held_to_its_language` say the same. For the missing receipt, the round reply carries report.json and the focused outputs inline, generated at the pushed head named there. Committed on top of 608a4af in the commit that adds this text.

## Unresolved limits

No experiment has been run. The lingua accuracy figures are the project's own German benchmark. Per-language charts are images, and there are no English/French numbers or independent benchmarks. Whatlang publishes no short-text accuracy. The Android binary and memory cost of feature-limited lingua is unmeasured. It is unknown whether the reviewer will accept a lenient detector rule that can still let a rare translation through, and whether the person prefers the spec relaxation.
