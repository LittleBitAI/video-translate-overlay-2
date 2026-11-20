# Sentence cues: execution receipts

What ran, on what, and what it printed. In review round 7 every check in
[Round 7: the checks at `066064e`](#round-7-the-checks-at-066064e) ran on commit
`066064e` (the F9 fix). The commit that carries this file follows it and
changes only this file and the F9 research page:
`git diff --name-only 066064e HEAD` prints
`docs/evidence/sentence-cues/receipts.md` and
`docs/research/review-sentence-cues-and-speech-cleanup-f9-1.md`, and neither
is read by any build, test or check. The sections after it are the round-5
receipts for `f25ed3d`, the last commit that changed application code, tests
or fixtures. `git diff --name-only f25ed3d 066064e` lists only the wiki
indexes (`.wiki/corpus.json`, `.wiki/graph.json`), the research pages, and
`docs/evidence/sentence-cues/` `README.md`, `places.md`, `receipts.md` and
`measure/check_readme.py`. Recorded on 2026-11-20, with a clean working tree
apart from the wiki's own untracked files.

## Environment

- Windows 11 Home 10.0.26200, x64
- rustc 1.98.1 (48a229cea 2026-09-01), cargo 1.98.1
- Node v24.18.0, Playwright 1.61.0 with its Chromium (headless)
- Rust builds with `VULKAN_SDK=C:/VulkanSDK/1.4.363.1` and the `whisper`
  feature only where noted.

## Round 7: the checks at `066064e`

Each row ran at `066064e811e4c9fc369f7f70f169c063b15290be` on 2026-11-20
(+0900), in the environment above, unless it says it is reused.

| Check | Command | Expected | Observed |
| --- | --- | --- | --- |
| Rust library suite (done items 2–7, 12–13) | `cargo test -q -p video-translate-overlay --lib` (11:59–12:03) | all pass; the 98 focused tests listed below still exist | `test result: ok. 269 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 197.71s`. `cargo test -q -p video-translate-overlay --lib -- --list` lists 271 tests, and every one of the 98 names below is among them |
| Contract host build | `cargo build -q -p video-translate-overlay --bin vto-contract-host` | builds | `target/debug/vto-contract-host.exe`, 25,928,192 bytes, written 12:03:44, SHA-256 `c96d5703fd87dfcc1a5364b7038f102d51163bdf628f08ca10805c460b2a2ad5`. The size matches the round-5 host; the hash differs because the binary was rebuilt |
| Contract tests (done items 3, 5–7, 13, 16) | `cargo test -p vto-contract-tests --test <file>` with that host as `VTO_CONTRACT_HOST` | 5, 5, 2, 7, 9 pass, with the 28 names below | sentences `ok. 5 passed` (16.21s); translation `ok. 5 passed` (46.57s); write_and_timing `ok. 2 passed` (3.44s); library `ok. 7 passed` (7.22s); jobs `ok. 9 passed` (20.26s). The 28 `ok` names are, sorted, the same as the 28 below (`diff` printed nothing) |
| Watch screen in Chromium (done item 9) | `npx vitest run -c vitest.ui.config.ts tests/ui/subtitles.test.svelte.ts` | all pass, including the long-cue test | `Tests 23 passed (23)`, including `a long Korean cue renders on two balanced lines at phone, windowed and fullscreen sizes` |
| Page overlay | `npx vitest run tests/overlay/placement.test.ts` | all pass | `Tests 5 passed (5)` |
| Line measurement (done item 10) | `npx vitest run -c docs/evidence/sentence-cues/measure/measure.config.ts` | `page overlay` and `watch screen` pass; the outputs equal `expected.json` | `Tests 2 passed (2)`. SHA-256 of the outputs: `measured-overlay.json` `d7374ad1…5dce9`, `measured-watch.json` `c3adb167…f1949`, byte for byte those of rounds 2 and 3 (table below); input `ko.json` `f6ad086f…16d0` |
| README against places.md (done items 14–15) | `python -I -X utf8 docs/evidence/sentence-cues/measure/check_readme.py` | `differences 0`; each negative control reported | `differences 0`. On the README before the round-7 fix: `differences 14`. Controls on scratch copies of places.md: a filler row set to other → differences 1; a row duplicated → differences 6; a term row set to "filler, term" → differences 2 (see [the F9 page](../../research/review-sentence-cues-and-speech-cleanup-f9-1.md)) |
| Wiki indexes (done item 17) | parse the committed `.wiki/corpus.json` and `.wiki/graph.json` (`git show 066064e:<path>`) and check their paths against `git ls-tree -r 066064e` | both parse; every indexed path exists; the graph is built from the corpus | Both parse (blob SHA-256 `707c0a23…c695` and `8df65902…5135`, last committed in `cb2d047`). 32 documents, all unique and tracked. `graph.nodes_from` is `.wiki/corpus.json`, `counts.docs` is 32. Every edge end outside the corpus is a tracked wiki page (`.wiki/<name>.md`, 7 pages including `subtitles-must-not-drift`), and every orphan is in the corpus. One tracked document is not indexed yet: the F9 research page, committed after the indexes were last built. The wiki has since rebuilt its working copies to 33 documents with that page included. The server commits those files, so this branch does not |
| Installed app (done item 16), reused | the run at `57f7173` below | the exe and the library it opened are the bytes that run used | Re-hashed at 12:05: the exe is still 75,448,832 bytes, written 2026-11-18 12:16:30 +0900, SHA-256 `7ff268da…ce0c`. The library is still 4,378,624 bytes, written 2026-11-19 12:10:18 +0900, SHA-256 `d4c1aded…7565`. Its log is still 238,992 bytes (13:20:09) and its index 32,768 bytes (21:31:39). `git diff --name-only 57f7173 066064e -- src tests` prints nothing. The app is not running |

## Focused offline checks (Rust unit tests)

The tests `cargo test -p video-translate-overlay --lib -- sentences:: stitch:: pipeline:: sync::library library store:: gemini:: api::`
(from `src-tauri`) selects, sorted by name (parallel runs print them in a
different order each time), as printed at `f25ed3d`. Against round 4,
`a_sentence_with_commas_goes_whole_however_long_and_only_a_bare_stretch_ends_at_the_limit`
is renamed `a_punctuated_sentence_goes_whole_and_only_a_bare_stretch_ends_at_the_limit`
(it gained the late-comma and end-mark-only forms), and seven tests are new
(marked †):

```
test account::tests::credential_expiry_keeps_local_library_ownership_and_account_switching_retires_that_scope ... ok
test account::tests::production_library_requests_select_the_account_and_copy_legacy_data_only_with_current_consent ... ok
test api::duration_tests::a_youtube_job_keeps_the_streams_length_not_the_players ... ok
test api::duration_tests::resumed_youtube_source_refreshes_unavailable_hints_but_preserves_refusals ... ok
test api::hold_tests::a_refused_check_lets_the_hold_go ... ok
test api::hold_tests::a_refused_hold_refuses_the_job_before_any_check ... ok
test api::hold_tests::the_service_is_held_before_the_media_check ... ok
test api::key_tests::the_mask_keeps_four_characters_and_never_splits_one ... ok
test api::tests::ids_and_titles_follow_the_entity_rules ... ok
test api::tests::privacy_opener_uses_only_the_fixed_public_page_and_preserves_account_and_video_state ... ok
test api::tests::video_keys_follow_the_video_not_the_url_form ... ok
test api::tests::youtube_sign_in_return_addresses_are_limited_to_youtube ... ok
test gemini::key_tests::a_400_that_names_the_key_is_a_refused_key ... ok
test gemini::key_tests::another_400_is_not_blamed_on_the_key ... ok
test gemini::tests::cost_is_usage_times_price_and_rate ... ok
test gemini::tests::glossary_is_deduplicated_and_sorted ... ok
test gemini::tests::missing_duplicate_or_foreign_ids_never_fill_subtitle_slots ... ok
test gemini::tests::replies_are_matched_by_id_only ... ok
test pipeline::audio_tests::a_failed_delete_keeps_the_path_for_the_next_try ... ok
test pipeline::audio_tests::audio_left_after_writing_is_deleted_on_the_next_round ... ok
test pipeline::boundary_tests::a_job_made_before_plans_resumes_on_its_own_windows ... ok
test pipeline::boundary_tests::a_remake_that_failed_writing_cannot_continue_or_be_retranslated_and_playback_keeps_the_selected_version ... ok
test pipeline::boundary_tests::a_write_killed_after_a_held_boundary_resumes_to_the_same_cues ... ok
test pipeline::boundary_tests::a_write_killed_with_a_sentence_open_across_a_cut_resumes_to_the_same_cues ... ok
test pipeline::boundary_tests::a_write_killed_with_a_sentence_open_loses_no_word_when_the_released_build_resumes_it ... ok
test pipeline::boundary_tests::a_write_the_released_build_advanced_resumes_without_its_stale_open_sentence ... ok
test pipeline::boundary_tests::every_spoken_word_is_written_once_in_order_and_in_time ... ok
test pipeline::boundary_tests::the_model_is_released_when_writing_ends_either_way ... ok
test pipeline::download_tests::the_first_attempt_uses_the_probed_source_and_reports_the_model_fetch ... ok
test pipeline::glossary_tests::a_job_with_no_glossary_fails_before_any_batch ... ok
test pipeline::glossary_tests::an_unusable_glossary_reply_is_asked_for_again ... ok
test pipeline::tests::every_stop_maps_to_one_of_the_e2_reasons ... ok
test pipeline::translate_tests::a_continued_remake_publishes_its_draft_and_a_restored_one_cannot_continue ... ok
test pipeline::translate_tests::a_refused_remake_leaves_the_audio_of_an_interrupted_one_as_it_was ... ok
test pipeline::translate_tests::a_selected_version_is_translated_with_its_own_terms_and_mishearings_found_for_it ... ok
test pipeline::translate_tests::a_stopped_remake_finds_the_version_it_started_from_in_the_database_alone ... ok †
test pipeline::translate_tests::an_older_builds_selection_never_pairs_a_transcript_with_another_ones_mishearings ... ok
test pipeline::translate_tests::cost_is_metered_per_call_and_never_stops_translation ... ok
test pipeline::translate_tests::cues_are_translated_one_per_request_with_requests_overlapping ... ok
test pipeline::translate_tests::every_translation_of_the_same_cues_asks_the_same_sentences ... ok
test pipeline::translate_tests::of_remakes_staged_at_once_under_one_job_id_one_wins_and_keeps_its_own_audio ... ok †
test pipeline::translate_tests::only_a_remake_that_failed_translating_continues ... ok
test pipeline::worker_tests::a_bug_fails_the_job_instead_of_leaving_it_running ... ok
test pipeline::worker_tests::a_job_in_an_unselected_account_resumes_in_that_database_and_publishes_its_completed_result ... ok
test pipeline::worker_tests::a_job_resumed_before_its_notice_went_out_notifies_once ... ok
test pipeline::worker_tests::every_ten_percent_step_passed_is_reported_once ... ok
test pipeline::worker_tests::failed_retranslation_preserves_completed_subtitles_for_every_stop ... ok
test pipeline::worker_tests::retranslation_resumes_partial_work_and_publishes_only_a_complete_set ... ok
test pipeline::worker_tests::transient_retries_are_independent_for_glossary_and_each_cue ... ok
test sentences::tests::a_bare_stretch_before_a_long_pause_still_ends_at_the_limit ... ok †
test sentences::tests::a_long_silence_ends_a_sentence_without_punctuation ... ok
test sentences::tests::a_punctuated_sentence_goes_whole_and_only_a_bare_stretch_ends_at_the_limit ... ok
test sentences::tests::a_saved_cue_holding_a_sentence_end_closes_its_sentence ... ok
test sentences::tests::a_segment_without_word_times_is_spread_over_its_time ... ok
test sentences::tests::a_sentence_ends_where_its_punctuation_ends_it ... ok
test sentences::tests::a_sentence_over_the_cap_is_cut_at_a_comma_then_a_conjunction_then_the_longest_pause ... ok
test sentences::tests::a_sentence_starting_mid_segment_starts_with_its_first_word ... ok
test sentences::tests::a_sentence_written_across_windows_is_grouped_as_in_one ... ok †
test sentences::tests::a_stretch_without_punctuation_is_cut_at_pauses_and_the_cap ... ok
test sentences::tests::after_an_initialism_etc_or_an_ellipsis_a_lower_case_word_carries_the_sentence_on ... ok †
test sentences::tests::cues_keep_after_the_previous_one ... ok
test sentences::tests::each_cue_is_one_sentence_and_none_holds_the_end_of_one_and_the_start_of_the_next ... ok
test sentences::tests::grouping_an_hour_of_unpunctuated_speech_is_quick ... ok †
test sentences::tests::no_cue_ends_after_its_words_even_when_they_take_no_time ... ok
test sentences::tests::no_request_holds_more_than_the_ceiling ... ok †
test sentences::tests::the_open_sentence_waits_for_the_next_window_and_ends_the_same ... ok
test sentences::tests::the_parts_of_a_sentence_are_asked_together_and_with_nothing_else ... ok
test stitch::tests::a_loop_on_a_sound_is_cut_and_said_repetition_and_numbers_stay ... ok
test stitch::tests::a_side_that_heard_nothing_in_the_overlap_gives_way_to_the_other ... ok
test stitch::tests::a_word_cut_by_the_window_edge_appears_once_and_the_split_sentence_is_one_cue ... ok
test stitch::tests::natural_repetition_across_the_cut_keeps_every_spoken_repeat ... ok
test stitch::tests::pieces_move_to_the_source_timeline_inside_their_window ... ok
test stitch::tests::segments_without_words_are_reconciled_as_whole_units ... ok
test stitch::tests::without_a_common_word_the_overlap_is_shared_at_its_middle ... ok
test store::library_tests::deletion_only_clears_the_browser_url_it_inspected ... ok
test store::library_tests::legacy_completed_rows_keep_timing_and_deletion_removes_all_owned_rows ... ok
test store::tests::a_job_from_a_file_without_plans_keeps_plan_one_and_new_jobs_get_the_current_plan ... ok
test store::tests::a_late_sign_in_does_not_replace_the_newer_opened_url ... ok
test store::tests::opening_a_current_file_does_not_wait_for_a_writer ... ok
test store::tests::region_save_is_atomic_and_percent_never_drops ... ok
test store::wal_tests::reads_do_not_wait_for_an_open_write ... ok
test sync::library::tests::active_jobs_defer_projection_and_confirmed_deletion_stays_hidden_until_cleanup ... ok
test sync::library::tests::concurrent_completed_versions_are_retained_and_fresh_restoration_waits_for_cleanup ... ok
test sync::library::tests::concurrent_order_ties_have_the_same_visible_order_on_both_replicas ... ok
test sync::library::tests::deletion_and_a_fresh_link_or_translation_in_one_page_cannot_bypass_cleanup ... ok
test sync::library::tests::failed_projection_rolls_back_received_records_and_cursor_together ... ok
test sync::library::tests::invalid_legacy_completion_rolls_back_the_copy_and_active_jobs_stay_local ... ok
test sync::library::tests::job_completion_cannot_adopt_a_deletion_received_after_the_job_started ... ok
test sync::library::tests::legacy_import_is_a_copy_is_idempotent_and_cannot_assign_another_accounts_store ... ok
test sync::library::tests::multi_place_moves_reach_the_peer_in_the_same_order ... ok
test sync::library::tests::native_completion_and_its_journal_commit_atomically_and_do_not_replace_the_local_job ... ok
test sync::library::tests::ordinary_library_mutations_publish_and_incoming_projection_does_not_echo ... ok
test sync::library::tests::partial_dependencies_survive_restart_and_blocked_rows_do_not_starve_later_work ... ok
test sync::library::tests::queued_remote_deletion_invalidates_earlier_consent_before_projection ... ok
test sync::library::tests::received_ready_library_is_usable_without_processing_and_survives_restart ... ok
test sync::library::tests::silent_cue_repair_denies_any_change_to_the_inspected_selection_or_cue ... ok
test sync::library::tests::silent_cue_repair_is_atomic_retains_versions_cost_and_sync_selection ... ok
test sync_channel::tests::different_signed_google_subjects_with_the_same_email_cannot_transfer_any_library_data ... ok
```

That is 98 tests, all `ok`: `test result: ok. 98 passed; 0 failed; 0 ignored; 0 measured; 173 filtered out; finished in 179.79s`.
The whole library suite, `cargo test -q -p video-translate-overlay --lib`, run
on the same tree, printed
`test result: ok. 269 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 192.64s`.

The new F5 and F6 tests were also run against the round-4 code, in a temporary
worktree at `5d74d11`, with this round's `sentences_tests.rs` and a test-only
`CEILING` constant. Four failed:
- `after_an_initialism_etc_or_an_ellipsis_a_lower_case_word_carries_the_sentence_on`
  split `It started at the U.S. this morning.` after 50 ms;
- `a_punctuated_sentence_goes_whole_and_only_a_bare_stretch_ends_at_the_limit`
  gave 2 sentences for the comma after twice the limit, and, run first, 3 for
  the end-mark-only form, where 2 were expected;
- `a_sentence_written_across_windows_is_grouped_as_in_one` gave 6 sentences
  where 3 were expected;
- `no_request_holds_more_than_the_ceiling` gave 1 sentence.

The F12 form (`short-gap` in
`a_write_the_released_build_advanced_resumes_without_its_stale_open_sentence`)
failed before the fix with `[word0, word1, word0, word1, word2, word3]`, and
`of_remakes_staged_at_once_under_one_job_id_one_wins_and_keeps_its_own_audio`
failed before the fix with "round 0: the winner's audio was replaced or
deleted". Both pass at `f25ed3d`. The worktree was removed.

## Isolated API checks (contract tests)

The contract host is this commit's `vto-contract-host`, built with
`cargo build -q -p video-translate-overlay --bin vto-contract-host` from the
tree committed as `f25ed3d` and passed as
`VTO_CONTRACT_HOST=C:/Users/dasdk/PycharmProjects/video-translate-overlay-2/target/debug/vto-contract-host.exe`:

- 25,928,192 bytes, written 2026-11-20 06:39:01 +0900;
- SHA-256 `0648599d035b50d9b4b4bceffc7593761b543b1124bac4813562234c86ce7ba3`.

The harness (`contract-tests/src/lib.rs`) gives each test its own folder,
`%TEMP%\vto-contract-<pid>-<nanos>-<n>`, for the host's data, keystore and
notifications, and starts its fake Gemini and media server on loopback ports
the system picks (`127.0.0.1:0`). The host's config points it there, so
nothing reaches the network or the account library. `a_split_sentence_goes_to_gemini_as_one_request_of_its_parts`
also checks that `GET /videos/{key}/subtitles` returns cues of exactly
`end_ms, seq, start_ms, text_ko`. Command:
`cargo test -p vto-contract-tests --test <file>`, for the five files, with
that host. No contract test changed in round 5
(`git diff --stat 5d74d11 f25ed3d -- contract-tests` prints nothing), so the
test names are those below; their result lines at `f25ed3d` were:

```
sentences: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.68s
translation: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.17s
write_and_timing: test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.69s
library: test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.34s
jobs: test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.04s
```

The names, as printed by the same five files at `b0b123c`:

```
== --test sentences
test a_retranslation_asks_for_each_sentence_as_the_first_translation_did ... ok
test a_split_sentence_goes_to_gemini_as_one_request_of_its_parts ... ok
test fillers_stay_in_the_saved_source_and_the_instruction_leaves_them_out_of_the_korean ... ok
test mishearings_found_with_the_glossary_are_stored_and_carried_in_every_request ... ok
test a_reply_with_a_missing_extra_or_duplicate_part_is_not_saved ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.65s
== --test translation
test retranslation_keeps_saved_source_times_and_the_cost_already_spent ... ok
test test_translation_makes_korean_cues ... ok
test test_cost_never_stops_translation ... ok
test test_terms_stay_consistent_across_batches ... ok
test test_one_hour_lecture_costs_under_budget ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.31s
== --test write_and_timing
test test_silent_span_has_no_cue ... ok
test test_cue_times_match_source_after_silence_trim ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.42s
== --test library
test moves_span_several_places_in_one_request_and_clamp_at_the_ends ... ok
test playlist_crud_order_and_untranslated_links_persist_without_paid_calls ... ok
test moves_at_the_integer_limits_clamp_instead_of_overflowing ... ok
test deletion_rejects_active_processing ... ok
test saved_library_reopens_after_restart_without_transcription_translation_or_cost ... ok
test deletion_clears_only_matching_open_video_urls ... ok
test deletion_confirmation_retry_and_multiple_memberships_preserve_unrelated_data ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.89s
== --test jobs
test test_job_without_key_asks_for_key ... ok
test test_undownloadable_video_shows_reason ... ok
test test_detect_button_starts_downloading_job ... ok
test test_subtitles_refused_until_ready ... ok
test test_repeat_detect_reuses_saved_subtitles ... ok
test test_other_site_media_follows_same_stages ... ok
test test_write_finishes_offline_on_device ... ok
test test_write_percent_and_ongoing_notice ... ok
test test_job_advances_in_background_and_resumes ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.60s
```

## Chromium checks and the line measurement

`npx vitest run -c vitest.ui.config.ts tests/ui/subtitles.test.svelte.ts` (the
Watch screen), `npx vitest run tests/overlay/placement.test.ts` (the page
overlay), and the measurement
`npx vitest run -c docs/evidence/sentence-cues/measure/measure.config.ts`
(each with `--reporter=verbose`). The measurement
([measure/measure.test.svelte.ts](measure/measure.test.svelte.ts)) renders
every Korean cue of the eight videos, old (before the task) and new (as saved
now), in both surfaces at the three sizes. It asserts that the count of cues
on each number of lines, and the [seq, lines] of every cue over two lines,
equal [measure/expected.json](measure/expected.json) for every surface, size,
version and video. It also asserts that the new versions have no cue over two
lines except 12 in the Watch screen on a phone.

This run is reused from round 3: rounds 4 and 5 change no UI code, harness,
cue text or contract test (`git diff --stat ee4359c f25ed3d -- src tests vitest.ui.config.ts docs/evidence/sentence-cues/measure`
and `git diff --stat b0b123c f25ed3d -- contract-tests src tests` print nothing), and the measured Korean comes from the saved cues, which it
does not touch. The run was made at `ee4359c1c57007f74cbb83fd33546eb8fae72dac`. Right before
it, `git status --porcelain` for the harness and everything it imports
printed nothing, so each ran as committed, with these git blob ids
(`git ls-files -s`):

| File | Blob |
| --- | --- |
| `docs/evidence/sentence-cues/measure/measure.test.svelte.ts` | `6602737074e7a5e2116e3d98fe582154677458d4` |
| `docs/evidence/sentence-cues/measure/measure.config.ts` | `ceb802aba2b4cb8b6f1880ba49f5724ec51b3040` |
| `docs/evidence/sentence-cues/measure/expected.json` | `43b3dc7b8c6a6104ebb917006f12721aa7c1a809` |
| `docs/evidence/sentence-cues/measure/check_readme.py` | `c7f29f93622ee4f4519825390a0a270caf9a5bad` |
| `vitest.ui.config.ts` | `ddab02f7244ff248ddda23770a8be57df2d8d1cb` |
| `tests/subtitle-lines.ts` (`lineWidths`) | `d234f9fb599271c5fad6124736d9fd120b5fb23a` |
| `tests/ui/fake.ts` | `605b0396e7c94ebab01bf2ce0981ea72c7bc816f` |
| `src/ui/Overlay.svelte` (the Watch screen) | `493a936eab936e4a392a6bf2b5e4e61ba781aca6` |
| `src/overlay/index.ts` (the page overlay) | `c6bd0a623baffbe0b490ecca55ff349d9a1c552e` |
| `src/ui/app.css` | `3b12cec4934898776e3c8e18f1906e94a1adb477` |

None of them changed between `5c76696`, where the harness was committed, and
`ee4359c` (`git diff --stat 5c76696 ee4359c -- docs/evidence/sentence-cues/measure src tests vitest.ui.config.ts`
prints nothing).

```
== npx vitest run -c vitest.ui.config.ts tests/ui/subtitles.test.svelte.ts --reporter=verbose
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > local subtitles scale with their player and wrap inside a letterboxed image 247ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > a long Korean cue renders on two balanced lines at phone, windowed and fullscreen sizes 69ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deleting another video preserves cues through a failed refresh: native ready 1054ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deleting another video preserves cues through a failed refresh: native failed 1060ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deleting another video preserves cues through a failed refresh: broadcast ready 1018ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deleting another video preserves cues through a failed refresh: broadcast failed 1059ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > a late subtitle response after deletion cannot restore cached cues: native ready 155ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > a late subtitle response after deletion cannot restore cached cues: native failed 188ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > a late subtitle response after deletion cannot restore cached cues: broadcast ready 170ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > a late subtitle response after deletion cannot restore cached cues: broadcast failed 173ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deletion before the first job response cannot revive that video: native 1163ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deletion before the first job response cannot revive that video: broadcast 1170ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > confirmed recreation replaces earlier deletion markers: native known identity 173ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > confirmed recreation replaces earlier deletion markers: native unknown identity 219ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > confirmed recreation replaces earlier deletion markers: broadcast known identity 166ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > confirmed recreation replaces earlier deletion markers: broadcast unknown identity 209ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deletion after consent still rejects a late creation response: native 166ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deletion after consent still rejects a late creation response: broadcast 154ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > the cue under the playback time, hidden while an ad plays 54ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > failed jobs recover saved subtitles and keep the failure reason: initial, saved=true 66ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > failed jobs recover saved subtitles and keep the failure reason: initial, saved=false 52ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > failed jobs recover saved subtitles and keep the failure reason: poll, saved=true 1052ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > failed jobs recover saved subtitles and keep the failure reason: poll, saved=false 1051ms
 Test Files  1 passed (1)
      Tests  23 passed (23)
== npx vitest run tests/overlay/placement.test.ts --reporter=verbose
 ✓ |chromium| tests/overlay/placement.test.ts > test_overlay_lies_over_the_video_and_follows_it_into_full_screen 83ms
 ✓ |chromium| tests/overlay/placement.test.ts > a long Korean cue renders on two balanced lines at phone, windowed and fullscreen sizes 20ms
 ✓ |chromium| tests/overlay/placement.test.ts > test_a_video_too_small_to_hold_it_puts_the_overlay_at_the_bottom 8ms
 ✓ |chromium| tests/overlay/placement.test.ts > ready subtitles stay inside a landscape fullscreen player and survive returning to portrait 21ms
 ✓ |chromium| tests/overlay/placement.test.ts > the fullscreen button requests the player with its saved subtitles, then exits 2ms
 Test Files  1 passed (1)
      Tests  5 passed (5)
== npx vitest run -c docs/evidence/sentence-cues/measure/measure.config.ts --reporter=verbose
 ✓ |chromium| docs/evidence/sentence-cues/measure/measure.test.svelte.ts > page overlay 5963ms
 ✓ |chromium| docs/evidence/sentence-cues/measure/measure.test.svelte.ts > watch screen 4399ms
 Test Files  1 passed (1)
      Tests  2 passed (2)
```

The files the run wrote and read, by SHA-256. The two outputs are byte for
byte those of the round-2 run at `5c76696`:

| File | SHA-256 |
| --- | --- |
| `docs/evidence/sentence-cues/measure/expected.json` (committed; asserted equal) | `1eceedeed6dc700ca89e5a19d5b9ece340996da5d24a9db88e14e4aec9448905` |
| `artifacts/sentence-cues/measured-overlay.json` (written by the run) | `d7374ad1f0548be4e0607e6ad8763b48fcb226a4997a6fc1404b92300ee5dce9` |
| `artifacts/sentence-cues/measured-watch.json` (written by the run) | `c3adb167ee568013bc4636a2830b69b86d03580396bf666e30f73f3b400f1949` |
| `artifacts/sentence-cues/ko.json` (input, not committed) | `f6ad086f05c8522d8f9ae89689cd427014da8fd59eac290ec43a0be4f0ae16d0` |

`python -I -X utf8 docs/evidence/sentence-cues/measure/check_readme.py`
checks that the README's rendered-line tables are sums of `expected.json`
and that every exception it holds is listed. Since review round 6 it also
counts the verdicts in `places.md` again. It compares them with the reading
summary rows, the All 8 row, and each video's place count and tally, and it
checks that every place judged worse is listed under Regressions (within 10 s
of the time named there, the same rule `places.md` uses to place an old cue's
time). At the fix commit, the one after `d079478`, it printed `differences 0`.

On the README committed before that fix (`d079478`), the same script printed
`differences 5`:
- the `WjhDDeZ7DvM` summary row, tally and place count;
- the All 8 row;
- `_MEqhFwnp8o` 30:18, a place judged worse that was not listed.

Negative controls on scratch copies of `places.md` were each reported:
- one verdict flipped: 3 differences;
- one row removed: 4;
- one row duplicated: 4;
- a video renamed: 4;
- one place changed from same to worse: 4, including "worse place not listed under Regressions".

Line counts from that run (cues on 1 / 2 / more than 2 lines; the most lines
one cue took). They are the same as the tables in [README.md](README.md):

| Surface | Size | Old | New |
| --- | --- | --- | --- |
| Page overlay | windowed PC 1280×800 | 2,064 / 2,092 / 137 (7) | 6,828 / 288 / 0 (2) |
| Page overlay | fullscreen PC 1920×1080 | 2,098 / 2,061 / 134 (7) | 6,845 / 271 / 0 (2) |
| Page overlay | phone 384×854 | 1,261 / 2,396 / 636 (9) | 5,582 / 1,534 / 0 (2) |
| Watch screen | windowed PC 1280×800 | 2,046 / 2,105 / 142 (7) | 6,817 / 299 / 0 (2) |
| Watch screen | fullscreen PC 1920×1080 | 2,089 / 2,070 / 134 (7) | 6,838 / 278 / 0 (2) |
| Watch screen | phone 384×854 | 997 / 2,210 / 1,086 (10) | 4,791 / 2,313 / 12 (3) |

The cue texts came from the account library: the new ones from its cue
table, the old ones from the backup taken before the first re-make. They are
not committed, because they are the person's library. Their file
(`artifacts/sentence-cues/ko.json`, 7,116 new and 4,293 old cues) had SHA-256
`f6ad086f05c8522d8f9ae89689cd427014da8fd59eac290ec43a0be4f0ae16d0`. The UI
code that renders them (`src/`, `tests/`) is the same at `3f8c8ed`,
`57f7173`, `5c76696`, `ee4359c`, `b0b123c` and `f25ed3d` (`git diff --stat 57f7173 f25ed3d -- src/ tests/`
prints nothing).

## The installed Windows app

This run is reused from the code at `57f7173`, by this task. None of
`5c76696`, `ee4359c`, `b0b123c` and `f25ed3d` changes what the released app
runs: the app is the installed 0.1.9 build, and the library it opened is
unchanged since. Re-checked on 2026-11-20 after `f25ed3d`: the exe's SHA-256
is still `7ff268da…ce0c`; the library file is still 4,378,624 bytes, last
written 2026-11-19 12:10:18 +0900, SHA-256
`d4c1adedf23a2dd5fe9ff790ba46d6e24a5b20953f335e50787da9471ca77565`. Its
write-ahead log (238,992 bytes, last written 13:20:09 +0900) holds that app
session's writes. The app is not running. The unchanged subtitle shape at
`f25ed3d` is shown by the contract test above. The regroup and request-size
counts in the README were made on a copy of this library in the temporary
folder, taken with SQLite's backup from a read-only connection, and the copy
was deleted afterwards. That read updated only the shared-memory index file
(`sync.sqlite3-shm`, 2026-11-19 21:31:39 +0900); the database and its log are
as above.

The installed app is version 0.1.9:

- path `%LOCALAPPDATA%\video-translate-overlay\video-translate-overlay.exe`;
- FileVersion and ProductVersion 0.1.9, 75,448,832 bytes, written 2026-11-18 03:16:30Z;
- SHA-256 `7FF268DA80DCF2C8F8E563F82BB4997A7B05CCFE575F85ACA63A0348A1EECE0C`.

The account library it opened is
`%LOCALAPPDATA%\dev.vto.overlay\sync\9d06ffd2…5d956\sync.sqlite3`. Before
launch it was 4,378,624 bytes with an empty write-ahead log, SHA-256
`d4c1adedf23a2dd5fe9ff790ba46d6e24a5b20953f335e50787da9471ca77565`, and
`PRAGMA integrity_check` returned `ok`. In it, `wbc-oxFFV9c` has 2 saved
versions. The selected one is
`t/wbc-oxFFV9c/9d591a03…dac8506`, with 1,490 cues, the last ending at
4,563,180 ms. Its newest job is `remake-wbc-oxFFV9c-1795054292`: ready,
966.84 KRW.

The app was started with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333`
and driven over that port (`docs/evidence/transcription-speed/scripts/cdp.mjs`).
Its Library card for the video read "저장본 2개". Opening it showed, in the main
window, "자막 준비됨 · 966.8원" and the first cue "1시간만 투자하세요, 딥러닝의
핵심을 영원히 이해하게 해 드리겠습니다." In the video window (www.youtube.com),
the app's own commands returned, verbatim except for elision:

```
get_job      → {"stage":"ready","cost_krw":966.8386124999996,"video_key":"wbc-oxFFV9c"}
get_subtitles → top-level keys [account_generation, cues, duration_ms, source_language, video_key]
                1490 cues; cue keys [end_ms, seq, start_ms, text_ko]; duration_ms 4571846
                cue 0    {"seq":0,"start_ms":0,"end_ms":4800,"text_ko":"1시간만 투자하세요, 딥러닝의 핵심을 영원히 이해하게 해 드리겠습니다."}
                cue 1388 {"seq":1388,"start_ms":4246280,"end_ms":4250780,"text_ko":"먼저, GPT가 글을 쓸 때 무엇을 유지하며, 왜 그 크기가 커지는지 알아보겠습니다."}
                cue 1489 {"seq":1489,"start_ms":4561540,"end_ms":4563180,"text_ko":"이제 딥러닝이 이해되기 시작합니다."}
```

Played from 1,019.5 s and paused at 1,021,794 ms, the overlay's subtitle element
showed "이제 코드로 16개의 필터를 만들고 개수를 세어 봅시다." on 1 line. That is
the library's cue 327 (1,020,880–1,024,340 ms, "Let's build those 16 filters
in code and count."). The old version's cue at that time is "코드로 16개의
필터를 만들어서 세어 봅시다." (its cue 185 of 885). The app was then closed.
