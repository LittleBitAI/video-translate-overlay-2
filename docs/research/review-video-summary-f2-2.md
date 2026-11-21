# Review research — F2, cycle 2

Draft research before the fix of review round 5 of PR #23, written 2026-11-21. It is not a verified fact or a shared rule: the review reads it with the fix.

## Problem

F2: a topic is accepted only if it is written in the lecture's own scripts and in the lecture's own words, whatever the lecture's language code and whether or not its scripts put spaces between words. At 2c0cc1d an English lecture whose transcript is `Attention uses ベイズ推定 and weights.` accepts the topic `quantum optics`, which neither the transcript, title nor glossary says. The 3 kana letters are 3 of 28 letters, so they meet own_scripts' 10% threshold (summary.rs:97) and Kana joins the talk's own scripts. in_talk (summary.rs:131) then sets `spaceless` for the whole talk because any own script is unspaced, and the `spaceless ||` short-circuit at :132 skips the word overlap for every topic. That includes Latin topics, whose words are perfectly comparable.

Trigger: recurrence, rounds 1, 2, 3, 4, 5.

- [P1] [P1] src-tauri/src/summary.rs:131 — A quoted term in an unspaced script disables topic word checks for an English lecture

## Failed fixes

- Round 1 (bce7681 -> e606658): replaced the language-code table's skip with own_scripts' 10% letter tally and removed the ko skip. The tally was designed as a permissive allowance (which scripts a topic MAY use). That is safe for an allowance, but round 5 shows the same set later feeds an exemption.
- Round 2 (e606658 -> fdb5179): restricted the tally to the transcript, so glossary Korean targets no longer add Hangul. This held for scripts but kept the 'any script with 10% or more counts as own' semantics that round 5 exploits through the transcript itself.
- Round 3 (fdb5179 -> bd36588): added said_whole for topics with no words() word, placed before the spaceless branch. Correct for spaced scripts, but its same-script boundary rule refused short Han topics in Chinese talks (round 4).
- Round 4 / cycle 1 (bd36588 -> 2c0cc1d): introduced `unspaced(Script)` and made said_whole judge each neighbour by that neighbour's script rather than by the talk's. The same commit reused `unspaced` in in_talk as a talk-level flag: `own.iter().any(|&s| unspaced(s) && ...)`. The per-character principle was applied to boundaries but not to the exemption, so a 10% kana quote in an English talk now exempts every topic. The tests only combined unspaced scripts with talks that are mostly in them (ja/zh), never a spaced talk with a quoted unspaced term.

## Sources

### S1 — UAX #29: Unicode Text Segmentation (Unicode 18.0.0, Revision 49)

- URL: https://unicode.org/reports/tr29/
- Retrieved: 2026-11-21
- Locator: Section 1.2 Rule Constraints item 4 'Script boundaries'; Section 2 Conformance, closing paragraph
- Fragment: "implementations are free to customize boundary testing to break at script boundaries" ... "the Common/Inherited values need to be handled properly"; by default "the string 'aquaφοβία' is treated as a single word". "reliable detection of word boundaries in languages such as Thai, Lao, Chinese, or Japanese" "requires the use of dictionary lookup or other mechanisms". (Quoted through a fetch summarizer.)
- Applicability: Supports deciding segmentability per script run (each topic word) rather than per talk, and splitting words() at unspaced/spaced transitions in summary.rs:111.
- Counterevidence: The default rules do NOT break at script changes; breaking there is a permitted customization, not the default. Digits and signs (Common) beside a script change need care, e.g. `2次元`.
- Validation: The en+kana-quote row is refused and the ja `Transformerの仕組み` row is kept after the change.

### S2 — ICU User Guide: Break Rules, Word Dictionaries

- URL: https://unicode-org.github.io/icu/userguide/boundaryanalysis/break-rules
- Retrieved: 2026-11-21
- Locator: Word Dictionaries (under Rule Status Values)
- Fragment: "The break rules must select, as unbroken chunks, ranges of text to be passed off to the word dictionary for further subdivision." ... "a character class named $dictionary that contains the characters (letters) to be handled by the dictionary." ... "The dictionary implementation, on receiving a range of text, will map it to a specific dictionary based on script, and then delegate to that dictionary for subdividing the range into words."
- Applicability: A production segmenter treats dictionary-needing scripts as runs within otherwise rule-segmented text, which is the per-run model proposed for in_talk/words().
- Counterevidence: ICU then subdivides those runs with a dictionary; this code exempts them instead, which is a weaker stand-in.
- Validation: Same rows as S1.

### S3 — src-tauri/src/summary.rs at 2c0cc1d

- URL: src-tauri/src/summary.rs
- Retrieved: 2026-11-21
- Locator: own_scripts :83-100 (threshold :97), words :111-115, in_talk :127-133 (spaceless :131), unspaced :138-140, said_whole :147-156, tests :458-538 and :552-585
- Fragment: `if n * 10 >= letters && !matches!(s, Script::Latin | Script::Greek) && !own.contains(&s) { own.push(s); }`; `let spaceless = own.iter().any(|&s| unspaced(s) && !(s == Script::Han && own.contains(&Script::Hangul)));` `spaceless || words_of.iter().any(|w| source.contains(w) || ...)`; words(): `text.split(|c: char| !c.is_alphanumeric())`. No test row pairs a mostly spaced talk with an unspaced quote; (ja, topics, latin, true) uses the English transcript from source(_).
- Applicability: Locates the talk-level flag, the permissive tally it reads, and the tokenizer that would merge Latin into unspaced runs.
- Counterevidence: none given
- Validation: The new rows fail or pass as predicted at 2c0cc1d and after the change.

### S4 — docs/design/backend.md, summary validation

- URL: docs/design/backend.md
- Retrieved: 2026-11-21
- Locator: lines 203-217
- Fragment: "For a talk written with spaces between words (every one but those whose own scripts are Han without Hangul, kana, Thai, Lao, Myanmar or Khmer), every topic must also share a word with the title, the glossary's source terms or the transcript ... A topic with no such word (`AI`, `Go`, `C++`), in any talk, must be said whole ... In Han, kana, Thai, Lao, Myanmar or Khmer nothing marks a word end without a dictionary, so `模型` is said in `语言模型训练`, and a short topic inside a longer compound of those scripts is accepted too"
- Applicability: The design writes the exemption per talk via 'own scripts', and own scripts include any 10% script. So the round-5 hole is in the design text, not just the code, and the paragraph must change with the code.
- Counterevidence: Read literally, the design exempts a talk whose own scripts include kana, so 2c0cc1d follows its spec. The spec, not only the code, is what is wrong.
- Validation: The paragraph is reworded per word in the same change.

### S5 — Review research — F2, cycle 1

- URL: docs/research/review-video-summary-f2-1.md
- Retrieved: 2026-11-21
- Locator: Applicability; Hypothesis; Result
- Fragment: "The neighbour test should look at the neighbour character's script, not the talk's" ... "Define that set once and use it in both `said_whole` and the `spaceless` predicate in `in_talk`."
- Applicability: Shows the cycle-1 fix applied the per-script principle to neighbours but explicitly prescribed reusing it as a talk-level predicate, which is the gap round 5 found.
- Counterevidence: Cycle 1's recommendation was reasonable for talks mostly in unspaced scripts, its only test axis.
- Validation: n/a (history)

## Claims

- C1 (source; S1): Reliable word boundaries in Thai, Lao, Chinese or Japanese need dictionary lookup, and implementations may break at script boundaries if Common/Inherited characters are handled properly.
- C2 (interpretation; S1, S2): Segmentability is a property of a script run within a text, not of the whole text: ICU passes ranges of dictionary characters to a dictionary chosen by script and segments the rest by rule.
- C3 (interpretation; S3, S4): At 2c0cc1d an English transcript with 3 kana among 28 letters gives the talk Kana as an own script, in_talk sets spaceless for the whole talk, and an unsaid Latin topic such as `quantum optics` is accepted.
- C4 (interpretation; S3, S5): The fixes kept failing because each applied a talk-level or allowance-level decision where a word-level one was needed, and each was tested only on its finding's axis; cycle 1 made boundaries per-character but kept the exemption per-talk.
- C5 (interpretation; S3, S4): Exempting only topic words written wholly in an unspaced script, plus splitting words() at unspaced/spaced transitions, refuses the round-5 case while keeping Latin terms in Japanese text matchable and preserving the decided limit for long Han/kana topics.

## Core theory

UAX #29 says reliable word boundaries in Thai, Lao, Chinese or Japanese require dictionary lookup or other mechanisms. It lets implementations break at script boundaries, provided Common/Inherited characters are handled properly. ICU implements this by isolating runs of dictionary characters and mapping each run to a dictionary chosen by its script. So segmentability belongs to a script run, not to a document. Text in a spaced script stays comparable by word even inside a document that also contains unspaced runs, and vice versa.

## Applicability

summary.rs:131-132 is the only place that still decides segmentation per talk; said_whole (:147-156) already decides per neighbour character via `unspaced`. own_scripts (:83-100) is correct as an allowance for in_scripts (:105) and should stay permissive; it should stop driving the exemption. words() (:111-115) splits only on non-alphanumerics, so in unspaced text it yields mixed-script tokens such as `transformerの仕組みを説明します`. It needs a split at unspaced/spaced transitions for Latin words in ja/zh transcripts to be source words. backend.md:203-205 describes the exemption per talk ('a talk written with spaces ... every one but those whose own scripts are Han without Hangul, kana, ...') and must be reworded per word in the same change.

## Hypothesis

Whether a topic's words can be compared to the talk's words is a property of the script each topic word is written in, not of the talk. in_talk decides it per talk from own_scripts, a permissive 10%-threshold allowance set, so any spaced-script talk that quotes enough Han/kana/Thai etc. loses the word check for all topics. Fix: drop the talk-level `spaceless` flag. Exempt a topic word from the overlap only if all of its letters are in an unspaced script (keeping the existing exception that Han in a talk whose own scripts include Hangul is not exempt). Latin and other spaced-script words must still be found in `source`. Also make words() split at transitions between unspaced and other characters, so Latin terms embedded in Japanese/Chinese text (`Transformerの仕組み`) become source words. Without that split, the per-word rule would wrongly refuse them.

## Counterevidence

- Behaviour change: in a Japanese or Chinese talk, a Latin topic of 3+ letters that the talk never says would now be refused (it passes at 2c0cc1d). This is consistent with round 3's refusal of an unsaid `Go` in a ja talk and with backend.md's 'named with words the talk itself uses', but it is a tightening, and a false refusal costs a metered 502.
- A Japanese transcript may write an English term only in katakana (`トランスフォーマー`) while the model names the topic in Latin (`Transformer`). The per-word rule refuses it unless the title or a glossary source term has the Latin spelling. No test covers this yet; the existing row (ja, topics, `Bayesian inference 2.0`, true) uses an English transcript.
- The per-word exemption still lets an unspaced-script topic word pass unsaid when the talk's scripts include that script. Example: an English talk with ≥10% kana and the topic `ベイズ`, which it never says. The topic's script is allowed and its word is exempt. This is narrower than today's hole but remains open.
- UAX #29 WB13 keeps Katakana runs together by default, so treating kana as never-segmentable is an approximation (already noted in cycle 1).

## Alternatives

- Talk-level flag from the dominant script: count a talk as unspaced only when unspaced letters are the majority (or above some threshold) of its letters. It fixes the round-5 example in one line, but leaves long Latin topics in ja/zh talks unchecked while short ones must be said whole (round 3), an asymmetry a later round is likely to raise. The threshold is arbitrary, and a Japanese talk heavy with English terms could fall below it. Second choice.
- Decide spacelessness from the language code (ja/zh/th...). Rejected: round 1 established that unlisted codes and None must not bypass checks, and speech recognition codes are not restricted to a table.
- Check unspaced-script topic words by substring containment in `said` instead of exempting them. It closes the residual hole (an unsaid kana topic in an English talk), but it reverses the decided limit that 3+ character Han/kana topics in a spaceless talk need not appear literally (backend.md:203-205; a Chinese topic may join words the talk says apart). Offer it to the person rather than adopting it silently.
- A dictionary segmenter (ICU4X icu_segmenter, jieba). Matches UAX #29's advice but adds a dependency and per-language data for a check whose limits are documented. Not first.

## Experiment

Before the fix, add rows to the tests. To each_field_is_held_to_its_language or a_short_topic_must_be_said_whole (with an explicit transcript): (en, `Attention uses ベイズ推定 and weights.`, topics `quantum optics`, false); this one should fail at 2c0cc1d and pass after. (en, same, `ベイズ推定`, true), (en, same, `weights`, true). (ja, `Transformerの仕組みを説明します。`, `Transformer`, true); it passes at 2c0cc1d and must still pass, which guards the words() split. (ja, same, `quantum optics`, false); this records the intended tightening. (zh, `今天讲解语言模型训练。`, `深度学习方法`, true); it keeps the decided limit for long unspaced topics. Then confirm every existing row keeps its result (ja/zh kana_han and han rows, the ko Hanja exception, the `Go`/`AI` rows). Then run the full gate: npm ci; node scripts/contract-tests.mjs artifacts/junit.xml; npm run test:ui; npm run build.

## Result

Observed in the commit that fills this Result (parent `7f246c4`; its rule code is that of `2c0cc1d`). The experiment's rows were added to `summary::tests::each_field_is_held_to_its_language` before the rule changed. That test now collects every row with the wrong result instead of stopping at the first. At the old rule it failed with three rows. Two were the predicted ones: `quantum optics` was kept in the English talk that quotes `ベイズ推定`, and in the Japanese `Transformerの仕組み` talk. The third was a wrong prediction: `ベイズ推定` in the English talk is refused at the old rule and the new one alike. The reason is the script allowance, not the word check: its two Han letters are 2 of 28, under a tenth, so Han is not one of the talk's scripts. The row now expects refusal and says why, and a kana-only row (`ベイズ`, kept) covers the quoted script. The other predicted rows held as expected: `weights`, ja `Transformer` and zh `深度学习方法` were kept.

The fix follows the hypothesis. In_talk's talk-level flag is gone. A topic word counts as shared without a match only if all its characters are in a script written without spaces, Hanja beside Hangul excepted. words() now also ends a word where such a script meets another. After it, `cargo test --lib summary::` gives 13 passed, and every earlier row of both tables keeps its result. With the split in words() disabled, only the ja `Transformer` row went wrong, so that row guards the split. The design text changed with the code: backend.md and api.yaml now describe the rule per word, not per talk. The full gate was not run here: the session leaves it to the server. The residual limit stays open and is documented in backend.md: a topic in a script without spaces that the talk never says is accepted when that script is a tenth of the talk's letters (`ベイズ` in a talk that quotes kana).

## Unresolved limits

Not executed in this session: no shell. The tally arithmetic (3 kana of 28 letters) and the code path were read, not run. How often real Japanese transcripts give English terms only in katakana while the model names them in Latin is unknown, so the false-refusal cost of the tightening is unmeasured. The residual hole (an unsaid unspaced topic word in a mostly spaced talk that quotes that script) remains unless the person chooses the substring alternative. The ICU per-script routing is quoted from the ICU user guide's description, not from its source code.
