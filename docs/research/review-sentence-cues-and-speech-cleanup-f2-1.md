# Review research — F2, cycle 1

Draft research before the fix of review round 4 of PR #22, written 2026-11-19. It is not a verified fact or a shared rule: the review reads it with the fix.

## Problem

Which saved cues are translated together in one Gemini request (one sentence, A6) must not depend on how the saved version reached the translator. Today the first translation uses the grouping the writer recorded (`job.parts`, from `sentences::push` -> `Cue.opens`, stored in `store.rs:501-523`). A version that is selected again or received by sync is projected without `parts` (`sync_store.rs:298-319`, and `sync::Cue` has no grouping field), so `pipeline.rs:463-466` falls back to `sentences::rebuilt_parts`, which runs `ends()` again on words rebuilt from the saved cue text, each word given its cue's time. The invariant 'rebuilt grouping == written grouping' fails whenever the writer's `ends()` decision depended on information the saved cues no longer hold. In round 4 that information is that `now` took zero time: the writer suppresses the end after `Done.`, the rebuild does not, and the 210-character cap then falls at a different seam.

Trigger: recurrence, rounds 1, 2, 3, 4.

- [P1] [P1] src-tauri/src/sentences.rs:348 — Re-selection can still join two groups that the current writer translated separately.

## Failed fixes

- Round 1 (3f8c8ed -> 1ff5752): copied `previous.parts` into the re-translation job (`store.rs:339-343`). That fixed the path that still had the recorded grouping, but selection and sync projection still have none and go through `rebuilt_parts`.
- Round 2 (1ff5752 -> 640b10c): made the comma rule look ahead (`bare`, `LOOK`) so writer and rebuild agree on comma sentences. That aligned one rule's inputs. The rebuild still sees different word times, durations and dropped words from the writer.
- Round 3 (640b10c -> 1dcd50f): added the zero-duration rule to `ends()` (`sentences.rs:88-101`): a word that ends no later than the word before it stays in that sentence, so the saved cues 'still show the sentence end it made'. The rebuild can never fire this rule, because words rebuilt from one saved cue share its start and take time (the comment at `sentences.rs:94-95` says so). Suppressing a writer end moves `from`, which changes `chars` for the CAP/STRETCH_MAX checks and the start of the `bare` lookahead. So the round 3 fix itself produced the round 4 divergence (`Done.` 0-500, `now` 500-500, then a bare run past 210 characters).

## Sources

### S1 — sentences.rs — ends() zero-duration rule and rebuilt_parts

- URL: src-tauri/src/sentences.rs
- Retrieved: 2026-11-19
- Locator: lines 68-113 (ends), 327-357 (rebuilt_parts)
- Fragment: "A word that ends no later than this one adds no time ... So it stays in this sentence, except as the very last word ... (Words rebuilt from one saved cue share its start and take time, so this never applies to them.)" and "The cues' words go through the same sentence ends as when they were written, each word at its cue's time: every end `push` makes falls on a cue boundary and depends only on the text and the pause there."
- Applicability: The code itself states that one rule applies to the writer and never to the rebuild, and it claims that ends depend only on text and pause. Lines 77-80 and 177 show that ends also depend on the length since `from`, which the suppression moves.
- Counterevidence: For inputs where no zero-time word and no inner-cue timing matters, the two paths do agree (the rebuilt_as_made tests pass).
- Validation: With grouping taken only from rebuilt_parts, the comment's claim no longer has to hold for correctness.

### S2 — sync.rs — wire types and translation identity

- URL: src-tauri/src/sync.rs
- Retrieved: 2026-11-19
- Locator: lines 36-62 (Cue, Translation), 103-126, 240-247 (identity)
- Fragment: `#[serde(deny_unknown_fields)] pub struct Cue { pub start_ms: i64, pub end_ms: i64, pub source_text: String, pub text_ko: String }` and `Change::Translation { translation } => format!("t/{}/{}", translation.video.key, digest(&serde_json::to_vec(translation).unwrap()))`
- Applicability: A synced version carries no grouping. Adding one changes the digest identity and is rejected by readers that deny unknown fields.
- Counterevidence: A coordinated release of every peer could accept the field, but phones on 0.1.9 exist.
- Validation: The recommended fix leaves the wire format unchanged, so existing sync tests remain valid.

### S3 — sync_store.rs — translation projection

- URL: src-tauri/src/sync_store.rs
- Retrieved: 2026-11-19
- Locator: lines 298-319
- Fragment: `DELETE FROM cue WHERE video_key=?` ... `INSERT INTO cue VALUES (?,?,?,?,?,?)` ... `INSERT INTO job(id,video_key,stage,percent,translation_model,cost_krw,created_at,updated_at) VALUES (?,?,'ready',100,?,?,...)` (no parts)
- Applicability: Every selected or received version reaches translation with `parts` NULL and therefore through rebuilt_parts.
- Counterevidence: none given
- Validation: The pipeline-level re-select test exercises this path.

### S4 — pipeline.rs — choice between recorded and rebuilt grouping

- URL: src-tauri/src/pipeline.rs
- Retrieved: 2026-11-19
- Locator: lines 461-467
- Fragment: "The sentences this job (or the job a re-translation came from) wrote; where none were kept, as for a projected saved version, they are rebuilt from the cues." `match job.parts ... { Some(p) => p.into_iter().collect(), None => crate::sentences::rebuilt_parts(&cues) }`
- Applicability: This is the one place where the two sources of truth meet. Making it always rebuild removes the second source.
- Counterevidence: none given
- Validation: With the match replaced by `rebuilt_parts(&cues)`, first translation and re-selection request the same groups.

### S5 — Serde container attributes

- URL: https://serde.rs/container-attrs.html
- Retrieved: 2026-11-19
- Locator: #[serde(deny_unknown_fields)]
- Fragment: "Always error during deserialization when encountering unknown fields. When this attribute is not present, by default unknown fields are ignored for self-describing formats like JSON." Note: "this attribute is not supported in combination with flatten, neither on the outer struct nor on the flattened field." And `default`: "When deserializing, any missing fields should be filled in from the struct's implementation of Default." (only allowed on structs)
- Applicability: Adding a grouping field to sync::Cue makes older readers error on the whole page. `default` only helps new readers read old records.
- Counterevidence: Readers built with the field and `default` accept old records, so the problem runs only from new writers to old readers.
- Validation: Not needed if the wire format stays unchanged.

### S6 — Derived Data — Systems Playbook

- URL: https://systems-playbook.pages.dev/concepts/derived-data/
- Retrieved: 2026-11-19
- Locator: Systems of record vs. derived data
- Fragment: Derived data is "distinguished from the system of record, the authoritative source of truth" and should be "defined as a function of the essential state rather than stored and mutated separately." "If derived data is lost, it can be rebuilt from the source of record." The page warns against "mistaking an irreplaceable store for derived and losing it." (Rebuilding works only if the source of record holds what is needed. The page implies this but does not spell it out.)
- Applicability: The writer's `opens` is treated as derived (rebuildable from cues), but it depends on state the cues do not hold, so it is effectively irreplaceable. Either store it in the record everywhere or define the grouping as a function of the cues only.
- Counterevidence: This is a secondary summary of Kleppmann's DDIA Part III. The original text could not be fetched (403/404).
- Validation: The round-trip property test shows directly whether the grouping is a function of the saved cues.

### S7 — Sentence cues evidence README

- URL: docs/evidence/sentence-cues/README.md
- Retrieved: 2026-11-19
- Locator: lines 45-54, 70-80
- Fragment: "Re-translating a selected re-made version keeps the grouping its re-make recorded. Only a version selected again or received from another device has its grouping rebuilt with the new rules. Against the recorded groupings, the rebuild joins 193 cues to the sentence before them (sentences the run-on limit had ended at a comma, ten of them sentences whose first comma comes after 210 characters) and splits 6 (a period before a lower-case word)."
- Applicability: On real data the two sources already disagree in 199 cues across eight videos, and the doc promises a behavior that the single-source fix would change.
- Counterevidence: Those differences are partly due to rule changes between recording and rebuild, not only to information lost from the cues.
- Validation: Recount after the fix: the recorded and rebuilt columns become the same thing.

## Claims

- C1 (source; S1): The writer's `ends()` applies a zero-duration suppression rule that, by the code's own comment, can never apply to words rebuilt from saved cues.
- C2 (interpretation; S1, S6): Because suppressing an end moves `from`, and the cap, stretch and lookahead checks count from `from`, a grouping computed by the writer cannot in general be recovered from the saved cues. No further change to `ends()` makes the rebuild exact.
- C3 (source; S2, S3): Selected and synced versions carry no grouping on the wire or in the projected job, so they always go through `rebuilt_parts`.
- C4 (interpretation; S4, S6): Making `pipeline.rs:463` always use `rebuilt_parts(&cues)` (and deleting `job.parts` writing and copying) gives every path the same function on the same input, so groups cannot diverge.
- C5 (interpretation; S2, S5): Persisting the grouping in `sync::Cue` would make older peers reject sync pages, because unknown fields are denied, and it would change translation identities.
- C6 (source; S6): Derived data must be defined as a function of the authoritative state, and that state must contain what the function needs.
- C7 (source; S7): On the eight saved videos the recorded and rebuilt groupings already differ in 199 cues, and the README promises that re-made versions keep their recorded grouping. The fix would change that promise.

## Core theory

Derived data is reliable only if it is defined as a function of the authoritative state and that state holds everything the function needs. When a derived copy and its source disagree, the source wins by definition. Here, after projection or sync, the saved cue rows are the only state that survives. The writer's word-level decision is state that does not survive, so a grouping that depends on it cannot be rebuilt from the cues.

## Applicability

`rebuilt_parts` (sentences.rs:327-357) claims that 'every end push makes falls on a cue boundary and depends only on the text and the pause there'. That claim is false: `ends()` also depends on word durations (lines 96-101), on the cumulative length since the sentence start `from` (line 77, 80), and on the `bare` lookahead measured from `from` (line 177). After projection and sync the only state left is the cue rows (`sync::Cue` = start_ms, end_ms, source_text, text_ko; `cue` table has no grouping column). If the grouping is defined as `rebuilt_parts(cues)` everywhere, pipeline.rs:463 is the single place it is computed, and the first translation, re-translation (store.rs:339-343), selection (sync_local.rs:192, library_store.rs:74) and sync projection (sync_store.rs:298-319) all feed it the same rows.

## Hypothesis

The grouping has two sources of truth: the writer's `opens`, computed from full word timing, and `rebuilt_parts`, computed from saved cue rows. The map from writer words to saved cues loses information (per-word times and durations, gaps inside a cue, zero-time words, parts that `sentence_cues` merged or dropped), and `ends()` reads exactly that information (`n.end_ms <= ws[i].end_ms`, start comparisons, gaps, char counts from `from`). So no change to `ends()` can make rebuild(saved(words)) equal writer(words) for every input. Each patch moves the disagreement somewhere else, as round 3 into round 4 shows. The fix that holds is to keep one copy: compute the grouping only from the saved cues (always `rebuilt_parts` at `pipeline.rs:463`, for the first translation too), and stop writing and copying `job.parts`.

## Counterevidence

- The README promises that 'Re-translating a selected re-made version keeps the grouping its re-make recorded' (docs/evidence/sentence-cues/README.md:74-75). The single-source fix drops that promise: those versions would get the rebuilt grouping (193 joined, 6 split across the eight videos per the same page). This is a behavior change the person may need to approve, though the joins are comma sentences the person decided in round 2 should go whole.
- The first-translation grouping would change wherever the writer's word-level rule mattered (e.g. the round 4 fixture would split at `Done.`). The receipts for the eight re-made videos were taken with the writer's grouping, so their request shapes may differ in edge cases.
- A further patch could in principle store extra hints in cue text or times so the rebuild can see the suppressed end. No such channel exists without changing what the cues show (text and times are user-visible and synced).

## Alternatives

- Persist the grouping with the saved data (an `opens` column on `cue` and a field on `sync::Cue`). Not first: `sync::Cue`, `Translation`, `Record` and `Page` are `deny_unknown_fields`, so older peers (0.1.9 phones) would fail `Page::decode` on any page carrying the field. The translation identity (`t/{key}/{sha256 of translation}`, sync.rs:243-247) would change for every new version. Records without the field would still need `rebuilt_parts`, so two sources of truth would remain.
- Another rule change in `ends()` (e.g. drop the round 3 zero-duration suppression, or make the rebuild emulate it). Not first: the zero-duration fact is not in the saved cues, so emulation is impossible. Dropping the suppression brings back the round 3 finding (the dropped zero-time `Okay.` hides the end after `No...`). This is the same patching pattern that failed four times.
- Compute the writer's `opens` by running `rebuilt_parts` on the cues it just produced and storing that in `job.parts`. Equivalent in outcome to the recommended fix, but it keeps the dead `job.parts` plumbing (store.rs:507-523, 339-343) as a second place that can drift. Deleting it is the smaller diff.

## Experiment

1) Show that patching cannot converge: a randomized round-trip test over word sequences mixing punctuation (., ..., ?, commas), zero-duration and overlapping words, and gaps from 0 to 3500 ms, with runs of 20-80 words. For each sequence run `push(.., last=true)`, map the cues to store::Cue as `rebuilt_as_made` does (sentences_tests.rs:138-142), and compare `rebuilt_parts` with the writer's `!opens` set. On the current code this is expected to find counterexamples beyond the round 4 fixture. 2) Confirm the fix: a pipeline-level test that writes the round 4 fixture (`Done.` 0-500, `now` 500-500, then 30 or more six-character words at short intervals), translates, then re-selects the version (`select_translation` / sync projection) and re-translates, recording the Gemini `translate` requests both times. With grouping taken only from `rebuilt_parts(cues)`, the two request lists must be identical. Repeat for the ellipsis fixture from round 3 and the comma fixture from round 2.

## Result

Observed with the fix (round 4):

1) Run on the round-3 code (`845191a`) in a temporary worktree, the randomized round trip (3,000 sequences of 20–80 words mixing `,` `.` `...` `?`, words with no time and overlapping words, gaps 0–3,500 ms, a fixed seed) found 109 sequences whose rebuilt grouping differs from the writer's. Patching `ends()` would not converge.
2) The grouping is now taken only from `rebuilt_parts(cues)` (`pipeline.rs`); `job.parts`, its column entry, its writing in `save_window` and its copy in `retranslate`, and `sentences::Cue.opens` are gone. `pipeline::translate_tests::every_translation_of_the_same_cues_asks_the_same_sentences` writes the round-4, round-3 (ellipsis) and round-2 (late comma) fixtures, translates, projects the saved version again (`restore_translation`), re-translates, and finds the same `translate` request groups. Run on the round-3 code, the same test fails on the round-4 fixture: the first translation (the writer's grouping) asked a four-part and a two-part request, and the re-translation (rebuilt) asked one six-part request.
3) Request sizes under rebuild-only grouping, on a read-only copy of the account library, exposed a second problem. Cues written under earlier rules hold sentence ends before their last word (`…in the image. lines`). The rebuild counted ends only at cue boundaries, so after a comma a sentence ran on through such ends, up to 21 parts and 43 s. Now a cue holding a sentence end anywhere closes its sentence (`sentences::tests::a_saved_cue_holding_a_sentence_end_closes_its_sentence`). After that, against the recorded groupings the rebuild joins 192 cues and splits 22. The median request size is unchanged; the largest is 18 parts, 763 characters, 40.9 s, and 5 requests run over 32 s. These are comma sentences with no end mark, which the person decided go whole. The per-video table is in `docs/evidence/sentence-cues/README.md`.
4) The README promise that re-translating a re-made version keeps its recorded grouping is replaced: its re-translation now asks the rebuilt grouping.
5) Review round 5 repeated the round-4 finding word for word. Rechecked at the round-5 head:
   - `every_translation_of_the_same_cues_asks_the_same_sentences` still passes.
   - The round-5 sentence-end changes (F5, F6) go through the same `ends()` that both the writer and `rebuilt_parts` call, so there is still one grouping.
   - `rebuilt_parts` now also stops at the 2,000-character ceiling. On the read-only library copy it joins 234 cues and splits 14 against the recorded groupings; the largest request is 967 characters and 22 parts.

## Unresolved limits

Not run: I can neither execute tests nor run a shell, so the property-test outcome on current code is predicted, not observed. I did not check whether `Cue.opens` from the writer feeds anything other than `job.parts` (display, held/tail resume). A grep showed only store.rs:514, but a reviewer should confirm. Whether dropping the README's 'keeps the grouping its re-make recorded' promise is acceptable is the person's decision. Rebuilt groups are bounded by the same STRETCH_MAX/LOOK rules, but I did not measure request sizes under rebuild-only grouping on the eight videos.
