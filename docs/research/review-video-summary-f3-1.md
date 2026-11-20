# Review research — F3, cycle 1

Draft research before the fix of review round 4 of PR #23, written 2026-11-21. It is not a verified fact or a shared rule: the review reads it with the fix.

## Problem

For a lecture with a known language, summary::check must store `topics` and `next_topics` only when every phrase is written in the lecture's language. Phrases in another language must be refused, and valid source-language terms (Latin names, Greek notation) must pass. The rule the reviews keep naming is that a search phrase uses the lecture's language. At 485fab7, `next_topics` are held only to which alphabet their letters come from (src-tauri/src/summary.rs:140), so a French phrase passes for an English lecture. The test at summary.rs:406 expects that pass.

Trigger: recurrence, rounds 1, 2, 3, 4.

- [P1] [P1] src-tauri/src/summary.rs:140 — The language check still accepts French recommendations for an English lecture

## Failed fixes

- Round 1 (22e5639 -> 13e28b4): refused Hangul in topics and next_topics for a non-Korean lecture. This checks only for one alphabet, not for the language, so Japanese and other alphabets passed. The new test computed its expected values with the same Hangul check, so it could not catch the gap.
- Round 2 (13e28b4 -> 817c63b): added a table of alphabets per language (in_scripts, summary.rs:76-96), allowing Latin and Greek letters in every language. It closed the Japanese-on-English example. But languages that share an alphabet cannot be told apart this way, so French passed for English. Greek letters had first been refused, which blocked valid terms like `β decay`.
- Round 3 (817c63b -> 485fab7): added Greek as always allowed and a word-overlap test (in_talk, summary.rs:110-114) requiring each topic to share a word of three or more letters with the title, glossary or transcript. That test cannot work for next_topics, which may name subjects the talk never mentions. So it was applied to `topics` only, and next_topics stayed on the alphabet check. The test table was written to expect the French recommendation to pass (summary.rs:405-406), pinning the bug the reviewer reported as correct behaviour.
- Across all three rounds, each fix was fitted to the reviewer's latest example and added a test row for it. No fix tested the general case (a same-alphabet translation, with or without accents), so the next reviewer example always found the open case.

## Sources

### S1 — src-tauri/src/summary.rs at 485fab7

- URL: src-tauri/src/summary.rs
- Retrieved: 2026-11-21
- Locator: lines 93-96, 110-114, 140-145, 404-406
- Fragment: fn in_scripts(...) { let Some(own) = scripts(language) else { return true }; text.chars().filter(|c| c.is_alphabetic()).all(|c| { let s = script(c); matches!(s, Script::Latin | Script::Greek) || own.contains(&s) }) } ... if field == "topics" && language.is_some_and(|l| !kept.iter().all(|t| in_talk(t, l, &source))) ... // Next topics come after the talk, so they are not held to its words: a same-script one passes. (Some("en"), "next_topics", accented, true),
- Applicability: Shows that next_topics get only the alphabet check and that the test expects a French recommendation to pass for an English lecture.
- Counterevidence: in_talk does close same-alphabet translation for topics, so only next_topics remain open.
- Validation: After the fix, the row at line 406 expects rejection and accent-free translation rows are added and refused.

### S2 — docs/design/backend.md, Video summary (P19)

- URL: docs/design/backend.md
- Retrieved: 2026-11-21
- Locator: lines 186-207
- Fragment: For a language written with spaces between words (the Latin, Cyrillic and Greek ones), every topic must also share a word of three or more letters with the title, glossary or transcript, so a same-script translation (French words for an English talk) is refused. Next topics, which come after the talk, are held only to the scripts: requiring the talk's words would refuse correct replies the person pays for.
- Applicability: The spec itself records the decision to leave next_topics on the alphabet check, and its reason (cost of false refusals).
- Counterevidence: The same section says next_topics are 'the lecture's language', which the alphabet check does not guarantee.
- Validation: The fix either changes this paragraph through spec-update or adds the detector rule here.

### S3 — lingua crate documentation (README), version 1.7.1

- URL: https://docs.rs/crate/lingua/1.7.1
- Retrieved: 2026-11-21
- Locator: accuracy section; minimum relative distance; low accuracy mode; from_languages
- Fragment: Lingua 'yields pretty accurate results on both long and short text, even on single words and phrases'. German: 73.9% single words, 94.1% word pairs, 99.7% sentences. 'the distance between the language probabilities is dependent on the length of the input text' ... do not set the minimum relative distance too high; None means 'language detection is not reliably possible'. Low accuracy mode: 'detection accuracy for short texts consisting of less than 120 characters will drop significantly'. All 75 languages are 'approximately 110 MB'; high accuracy mode with all models 'consumes approximately 1 GB of memory'. 'it is always a good idea to restrict the set of languages to be considered'.
- Applicability: The only kind of tool that can tell same-alphabet languages apart. Its short-input accuracy and memory cost set how lenient the rule must be.
- Counterevidence: Accuracy is self-reported on the project's own test data; there are no English/French figures in the text.
- Validation: The fixture experiment measures false refusals and misses on this project's phrases.

### S4 — lingua crate documentation, version 1.3.2

- URL: https://docs.rs/crate/lingua/1.3.2
- Retrieved: 2026-11-21
- Locator: accuracy figures (German)
- Fragment: German benchmark: 73.6% accuracy on single words, 94% on word pairs, 99.7% on sentences (the project's own test data).
- Applicability: Confirms the short-input accuracy ceiling across versions.
- Counterevidence: This is an older version; the figures differ slightly from 1.7.1.
- Validation: Same experiment as S3.

### S5 — docs/evidence/video-summary/README.md, paid run

- URL: docs/evidence/video-summary/README.md
- Retrieved: 2026-11-21
- Locator: lines 74-86
- Fragment: - mechanistic interpretability - reinforcement learning from human feedback - retrieval augmented generation - scaling laws - adversarial machine learning ... Prose fields came back in Korean; topics and next topics stayed in English, the lecture's language.
- Applicability: Real should-pass next_topics phrases for the experiment fixture; also shows the failure has not been observed live.
- Counterevidence: A single run; it does not show Gemini never translates.
- Validation: These five phrases must all pass under any new rule.

## Claims

- C1 (source; S1): At 485fab7, next_topics for a lecture in a Latin-alphabet language are checked only for Latin or Greek letters, and the test expects 'réseaux de neurones' to pass for `en`.
- C2 (interpretation; S1, S2): Each earlier fix answered only the reviewer's literal example with an alphabet or word-overlap check. Neither can decide which of the languages sharing an alphabet a short phrase is in, so the finding recurred.
- C3 (interpretation; S3, S4): Statistical language detection is the only code-level way to tell same-alphabet languages apart, but on one- or two-word inputs it is noticeably less accurate than on sentences, so a hard reject would refuse correct replies.
- C4 (interpretation; S2, S5): The spec left next_topics lenient to avoid refusing paid, correct replies. Tightening them or formally relaxing the requirement is a trade-off the person should decide if the detector experiment fails.

## Core theory

Lingua's documentation says that on short inputs, single words and word pairs, detection is noticeably less accurate than on sentences (its German benchmark: about 74% for single words, 94% for word pairs, 99.7% for sentences). It says the gap between language probabilities depends on input length, so a high minimum gap returns None ('detection is not reliably possible') for short text. It recommends restricting the candidate languages. An alphabet test carries no information about which of the languages sharing that alphabet a text is in.

## Applicability

summary.rs:93-96 (in_scripts) assigns no own alphabet to the 33 Latin-alphabet language codes, so for `en` any phrase made of Latin letters passes. summary.rs:110-114 (in_talk) applies only to topics (summary.rs:143). next_topics are by design phrases that may be absent from the transcript (backend.md:202-204), which rules out word overlap for them. A detector would apply at summary.rs:140 for the next_topics field. The candidate languages could be the talk's language plus a few other Latin-alphabet languages. The rule would reject only when another language beats the talk's language by a clear margin, so that short borrowed technical terms are not refused.

## Hypothesis

The validator tries to identify a short phrase's language using only its alphabet and word overlap with the talk. Neither can tell an English phrase from a French or Spanish one with no shared words, so any fix built on them leaves recommendations open. Only a statistical language detector can make that judgement in code, and it is unreliable on one- or two-word phrases. The fix therefore has to be either a lenient detector-based rule for next_topics, checked against real phrases, or an explicit spec decision by the person that next_topics are guaranteed only to be in the right alphabet.

## Counterevidence

- The paid run (docs/evidence/video-summary/README.md:74-86) shows Gemini already writing English next_topics for an English lecture, so the failure is hypothetical model behaviour, not something observed.
- Lingua reports about 74% accuracy on single words for its German benchmark: a hard reject would refuse correct replies the person pays for, which backend.md:203-204 names as the reason next_topics were left lenient.
- Technical English terms are used as-is in French or German talks (`deep learning`, `transformer`), so a detector may correctly call a valid phrase 'English' for a non-English talk, or call a borrowed term foreign.
- Lingua with all languages is about 110 MB of models and about 1 GB of memory in high-accuracy mode. The app also builds for Android, so dependency and memory cost may outweigh the benefit.

## Alternatives

- Accented-letter rule (for example, no non-ASCII Latin letters for `en`): closes the reviewer's literal example but not `apprentissage profond` or `aprendizaje por refuerzo`. It would be a fifth fix of the same kind, so it is not the first choice.
- Lists of common function words per language (de, la, des, der, und, del): cheap, but misses one-word and noun-only phrases. Still a heuristic arms race.
- Require next_topics to share a word with the talk: rejected because the reviewer states recommendations may introduce subjects outside the transcript.
- A second Gemini call to verify the language: backend.md fixes P19 as one call, and a second call doubles the cost; that is the person's decision.
- Spec change: the validator guarantees only the alphabet for next_topics, and the docs say a same-alphabet translation is not detected. This is honest and cheap but changes a stated requirement, so it needs the person through spec-update. It is the fallback if the detector experiment fails.

## Experiment

Build a fixture of next_topics phrases. Should pass: the paid run's five English recommendations (mechanistic interpretability; reinforcement learning from human feedback; retrieval augmented generation; scaling laws; adversarial machine learning), plus English technical terms used in fr/de talks. Should fail: 15 or more French, Spanish and German translations without accents (apprentissage profond, aprendizaje por refuerzo, tiefes Lernen, réseaux de neurones). Run the lingua crate restricted to the talk's language plus the other main Latin-alphabet languages, with a modest minimum gap. Adopt the detector rule only if zero 'should pass' phrases are refused and most 'should fail' ones are caught. Otherwise take the spec-change fallback to the person. In both cases, flip the expectation at summary.rs:406 or remove that row, and add the accent-free translations as rows.

## Result

pending — the fix records what it observed here, with the tests and the commit.

## Unresolved limits

No experiment has been run. The lingua accuracy figures are the project's own German benchmark; there are no English/French numbers or independent benchmarks. It is unknown whether lingua's per-language build options keep the Android build and memory acceptable. Whether the reviewer accepts a lenient rule that may still let a rare translation through is unknown.
