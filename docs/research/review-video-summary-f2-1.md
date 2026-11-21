# Review research — F2, cycle 1

Draft research before the fix of review round 4 of PR #23, written 2026-11-21. It is not a verified fact or a shared rule: the review reads it with the fix.

## Problem

F2: a topic is accepted only if it is written in the lecture's own scripts and is in the lecture's own words (src-tauri/src/summary.rs `check`/`in_talk`; docs/design/backend.md:195-216). The invariant: a topic the lecture actually says, in its own script, is accepted, and a topic it doesn't say is refused, whatever language code the lecture has and whether or not its language puts spaces between words. At bd36588 a Chinese lecture `今天讲解语言模型训练。` with topic `模型` is refused (502 summary_invalid, call metered). `words()` drops the 2-character Han word (it needs 3+ letters unless Hangul), so the topic goes to `said_whole`. `said_whole` refuses the match because `言` and `训` on either side are Han, the same script as the topic.

Trigger: recurrence, rounds 1, 2, 3, 4.

- [P1] [P1] src-tauri/src/summary.rs:129 — The short-topic check rejects valid topics in lectures written without spaces

## Failed fixes

- Round 1 (bce7681): the script table skipped unlisted codes (ar) and the word-overlap check skipped ko. The fix derived the lecture's scripts from a 10% letter tally (`own_scripts`) and removed the ko skip, so the table only adds scripts. The tally read title and glossary text too, which round 2 exploited.
- Round 2 (e606658): Korean glossary targets pushed Hangul over the 10% threshold for an English lecture. The fix made `judge` pass only the transcript to `own_scripts`, with the title and glossary source terms as `names` for words only. This held; the round 3 and 4 findings are not about scripts.
- Round 3 (fdb5179): a topic shorter than `words()` keeps (`Go`, `C++`) had no words, so `.any()` over an empty set... in fact `topic.is_empty()` let it pass. The fix added `said_whole` at the top of `in_talk`, applied in any talk, with a boundary rule (no same-script letter on either side). That rule assumes space-delimited words. It runs before the spaceless branch, and `words()`' 3-letter minimum makes most Chinese words and many kanji compounds 'short'. The only spaceless test row used a Latin acronym (`AI`/`Go` in a Japanese talk), where the neighbours are kana, a different script, so the flaw stayed hidden.
- Common pattern: each fix was checked only against the example in its finding. The property tables (`each_field_is_held_to_its_language`, `a_short_topic_must_be_said_whole`) don't combine the new branch with the other axes: spaced vs spaceless, the topic in the lecture's own script vs Latin, short vs long.

## Sources

### S1 — UAX #29: Unicode Text Segmentation (Unicode 18.0.0, revision 49 as reported by the fetch)

- URL: https://www.unicode.org/reports/tr29/
- Retrieved: 2026-11-21
- Locator: Section 4 Word Boundaries; 4.1.1 Default Word Boundary Rules WB13, WB999
- Fragment: "Indeed, some languages do not use spaces at all." For East Asian scripts ("Chinese, Japanese, Brahmi, Javanese, and others") and South East Asian scripts ("Thai, Myanmar, Khmer, and others"): "the default word boundary detection is not adequate; it needs tailoring." Conformance: "requires the use of dictionary lookup or other mechanisms". WB13: "Do not break between Katakana." WB999: "Otherwise, break everywhere (including around ideographs)." (Quoted through a fetch summarizer; Lao is named only under Tailoring, 6.5.)
- Applicability: Without a dictionary, a boundary inside Han text can fall anywhere, so `said_whole` must not treat a Han (or Thai etc.) neighbour as continuing the topic. Latin keeps its default letter-run rule.
- Counterevidence: WB13 keeps Katakana runs whole, so kana neighbours arguably do continue a word. The spec also says the default rules are inadequate, which means substring matching is only an approximation.
- Validation: The zh `模型` row and ja `確率` row pass. The `Go`-in-`Google` and ja `Go` rows stay refused.

### S2 — Rank-frequency relation for Chinese characters (arXiv:1309.1536)

- URL: https://arxiv.org/pdf/1309.1536
- Retrieved: 2026-11-21
- Locator: discussion of word length in modern Chinese (as surfaced by web search)
- Fragment: For modern Chinese word types single character words amount to some 10% against 66% of two-character words; some 54% of modern Chinese word tokens are single-character, two-character word tokens amount to 42%. (Counts depend on the lexicon; 'word' is hard to define in Chinese.)
- Applicability: Shows the 2-character Han case the finding names is the common case, not an edge: `words()`' 3-letter minimum sends most Chinese topics to `said_whole`.
- Counterevidence: The shares differ by source and method (a 2021 SAGE study gives 72% of 56,008 frequent words; Hu 1981 gives ~80% disyllabic).
- Validation: No code check; it motivates the test rows.

### S3 — src-tauri/src/summary.rs at bd36588

- URL: src-tauri/src/summary.rs
- Retrieved: 2026-11-21
- Locator: lines 111-115 words(), 127-133 in_talk(), 138-146 said_whole(), 533-560 a_short_topic_must_be_said_whole
- Fragment: `if words_of.is_empty() { return topic.chars().any(char::is_alphanumeric) && said_whole(topic, said); }` runs before `let spaceless = own.contains(&Script::Kana) || (own.contains(&Script::Han) && !own.contains(&Script::Hangul)) || ...`; said_whole: `(before.is_none() || before != first) && (after.is_none() || after != last)` where `kind` maps a character to its script; words(): `w.chars().count() >= if hangul(w) { 2 } else { 3 }`. The only spaceless test rows use `AI`/`Go` in a Japanese transcript.
- Applicability: This is where the faulty boundary rule and the branch order are. It is also where the shared spaceless-script set should go.
- Counterevidence: none given
- Validation: The new test rows fail at bd36588 and pass after the change.

### S4 — docs/design/backend.md, summary validation

- URL: docs/design/backend.md
- Retrieved: 2026-11-21
- Locator: lines 203-216
- Fragment: "For a talk written with spaces between words (every one but those whose own scripts are Han without Hangul, kana, Thai, Lao, Myanmar or Khmer), every topic must also share a word ... A topic with no such word (`AI`, `Go`, `C++`), in any talk, must be said whole ... not inside a longer run of the same script (`AI` in `AIの`, not in `MAIN`). ... A short topic the talk happens to say in another sense ... is not told apart: an accepted limit."
- Applicability: The design says 'in any talk' and 'not inside a longer run of the same script', but gives only a cross-script example. The design text should say the boundary rule applies to spaced scripts only.
- Counterevidence: The current wording, read literally, does require refusing `模型` inside Han text, so the design text must change along with the code.
- Validation: The design paragraph is updated in the same change as the code and tests.

## Claims

- C1 (source; S1): Unicode's default word segmentation is inadequate for Chinese, Japanese, Thai, Myanmar and Khmer. Real boundaries there need dictionary lookup, and the default rule breaks everywhere around ideographs.
- C2 (interpretation; S1, S3): So, without a dictionary, a same-script Han/Thai neighbour can't show that a short topic sits inside a longer word, and `said_whole` should ignore such neighbours while keeping the rule for spaced scripts.
- C3 (source; S2): Two-character words make up most modern Chinese vocabulary, so the refused case is the common one.
- C4 (interpretation; S3): At bd36588 a 2-character Han topic has no `words()`, takes the `said_whole` branch before the spaceless check, and is refused when Han characters sit on both sides of it.
- C5 (interpretation; S3, S4): The fixes kept failing because each new branch was tested only on its finding's example. The tables never combined short topics with spaceless talks in their own script, and the round 3 design wrote its boundary rule from a cross-script example only.

## Core theory

Unicode's default word-boundary algorithm says some languages do not use spaces at all, and for Chinese, Japanese and the South East Asian scripts the default detection is not adequate and needs tailoring, which means dictionary lookup or other mechanisms. Without one, the default rule breaks everywhere around ideographs (WB999). Only a run of Katakana is kept together (WB13). So without a dictionary, the only defensible boundary inside Han text is 'anywhere'. A same-script neighbour cannot mark a longer word. In Chinese, two-character words are the norm: about 66% of word types and 42% of tokens.

## Applicability

summary.rs:138-146 `said_whole` compares `kind(before)`/`kind(after)` with the topic's first/last script. That should hold only for spaced scripts, per UAX #29 WB999 for ideographs. summary.rs:112 `words()` puts the 3-letter minimum on Han, so the large disyllabic share of Chinese vocabulary goes to `said_whole`. summary.rs:129 runs that branch before the `spaceless` check at 130-131. The neighbour test should look at the neighbour character's script, not the talk's, so a Latin acronym in a Japanese talk keeps its Latin boundaries (`Go` not inside `Google`). That keeps the round 3 invariant, including the existing row 'never said in a talk without spaces' (ja, `Go`, false).

## Hypothesis

`said_whole`'s word-boundary test uses one notion of 'word' (a run of letters of one script, bounded by anything else) that only fits scripts written with spaces. For Han, kana, Thai, Lao, Myanmar and Khmer, a same-script neighbour says nothing about where a word ends. So a short topic in a spaceless lecture's own script is refused almost every time it appears. Fix: count a neighbour as continuing the word only if its script is written with spaces. Han, kana and blocks 0x1C/0x1D/0x20/0x21/0x2F should never count. Define that set once and use it in both `said_whole` and the `spaceless` predicate in `in_talk`.

## Counterevidence

- UAX #29 WB13 keeps Katakana runs together, so treating kana neighbours as never continuing will accept a 2-kana topic inside a longer katakana word (`ガス` in `ガスケット`). This is minor, because kana topics of 3+ letters already pass unchecked through the spaceless branch.
- Without a dictionary, a short Han topic found inside an unrelated longer Han compound is accepted. This is the same class as the already-accepted limit for the verb 'go' written `Go` (backend.md:214-216), but a reviewer could raise it.
- 3+ character Han/kana topics in a spaceless talk still pass without appearing at all (backend.md:203-205). After this fix, short ones must appear and long ones need not. That asymmetry was decided earlier and is not caused by this finding, but a later round could flag it.

## Alternatives

- Move the `spaceless` check above the short-topic branch. This fixes `模型` but lets an unsaid Latin `Go` pass in a Japanese/Chinese talk, which reopens round 3 (and its test row). Rejected.
- Lower `words()`' minimum for Han/kana to 1-2 letters like Hangul, so `模型` becomes a word and takes the spaceless pass. Smaller diff, but it loosens the check: a short Han topic the lecture never says would then pass, and the word set for Korean talks with Hanja changes. Second choice if the person prefers a uniform 'spaceless talks are not word-checked'.
- Use a dictionary segmenter (ICU/jieba) for Han/Thai. This matches UAX #29's advice, but it adds a dependency and per-language data for a check whose accepted limits are already documented. Not first.

## Experiment

Add rows to `a_short_topic_must_be_said_whole`: (zh, `今天讲解语言模型训练。`, `模型`, true), (zh, same, `数据`, false), (ja, `確率論の基礎とAIの話です。`, `確率`, true), (ja, same, `Go`, false, already there), (en, `The MAIN loop and Google use attention.`, `Go`, false, already there). Optionally add a Thai row with a short Thai topic inside a run of Thai. The first and third should fail at bd36588 and pass after the fix, and every existing row should keep its result. Then run the full gate (`npm ci`, `node scripts/contract-tests.mjs artifacts/junit.xml`, `npm run test:ui`, `npm run build`), not a single `cargo test`.

## Result

pending — the fix records what it observed here, with the tests and the commit.

## Unresolved limits

Script-of-neighbour is a heuristic and can't separate a real word from a substring inside a longer compound in spaceless scripts. Only a dictionary can (UAX #29). Treating kana as boundary-free goes against WB13 for Katakana runs. The Block-number set for Thai/Lao/Myanmar/Khmer (0x1C, 0x1D, 0x20, 0x21, 0x2F) is taken from the existing code and not re-checked against the Unicode block chart. The Chinese word-length shares vary by lexicon and by counting types vs tokens.
