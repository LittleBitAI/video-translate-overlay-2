# Sentence cues: execution receipts

What ran, on what, and what it printed, for the code at
`5c766960f4cf50826f57e2be7fa4f3a57087f050` (the last code commit of this
branch; later commits change only documents). Recorded on 2026-11-19, with
a clean working tree apart from the wiki's own files.

## Environment

- Windows 11 Home 10.0.26200, x64
- rustc 1.98.1 (48a229cea 2026-09-01), cargo 1.98.1
- Node v24.18.0, Playwright 1.61.0 with its Chromium (headless)
- Rust builds with `VULKAN_SDK=C:/VulkanSDK/1.4.363.1` and the `whisper`
  feature only where noted.

## Focused offline checks (Rust unit tests)

`cargo test --lib -- sentences:: stitch:: pipeline:: sync::library library store:: gemini:: api::`
(from `src-tauri`), every test line as printed:

```
test api::key_tests::the_mask_keeps_four_characters_and_never_splits_one ... ok
test api::tests::ids_and_titles_follow_the_entity_rules ... ok
test api::hold_tests::a_refused_check_lets_the_hold_go ... ok
test api::tests::video_keys_follow_the_video_not_the_url_form ... ok
test api::tests::youtube_sign_in_return_addresses_are_limited_to_youtube ... ok
test api::hold_tests::a_refused_hold_refuses_the_job_before_any_check ... ok
test api::tests::privacy_opener_uses_only_the_fixed_public_page_and_preserves_account_and_video_state ... ok
test account::tests::credential_expiry_keeps_local_library_ownership_and_account_switching_retires_that_scope ... ok
test gemini::tests::cost_is_usage_times_price_and_rate ... ok
test gemini::tests::glossary_is_deduplicated_and_sorted ... ok
test gemini::tests::missing_duplicate_or_foreign_ids_never_fill_subtitle_slots ... ok
test gemini::tests::replies_are_matched_by_id_only ... ok
test gemini::key_tests::another_400_is_not_blamed_on_the_key ... ok
test api::duration_tests::a_youtube_job_keeps_the_streams_length_not_the_players ... ok
test api::hold_tests::the_service_is_held_before_the_media_check ... ok
test gemini::key_tests::a_400_that_names_the_key_is_a_refused_key ... ok
test pipeline::audio_tests::a_failed_delete_keeps_the_path_for_the_next_try ... ok
test pipeline::audio_tests::audio_left_after_writing_is_deleted_on_the_next_round ... ok
test account::tests::production_library_requests_select_the_account_and_copy_legacy_data_only_with_current_consent ... ok
test pipeline::boundary_tests::a_remake_that_failed_writing_cannot_continue_and_playback_keeps_the_selected_version ... ok
test api::duration_tests::resumed_youtube_source_refreshes_unavailable_hints_but_preserves_refusals ... ok
test pipeline::download_tests::the_first_attempt_uses_the_probed_source_and_reports_the_model_fetch ... ok
test pipeline::glossary_tests::a_job_with_no_glossary_fails_before_any_batch ... ok
test pipeline::tests::every_stop_maps_to_one_of_the_e2_reasons ... ok
test pipeline::boundary_tests::every_spoken_word_is_written_once_in_order_and_in_time ... ok
test pipeline::boundary_tests::a_write_killed_after_a_held_boundary_resumes_to_the_same_cues ... ok
test pipeline::boundary_tests::a_write_killed_with_a_sentence_open_across_a_cut_resumes_to_the_same_cues ... ok
test pipeline::glossary_tests::an_unusable_glossary_reply_is_asked_for_again ... ok
test pipeline::boundary_tests::a_job_made_before_plans_resumes_on_its_own_windows ... ok
test pipeline::translate_tests::a_selected_version_is_translated_with_its_own_terms_and_mishearings_found_for_it ... ok
test pipeline::translate_tests::a_continued_remake_publishes_its_draft_and_a_restored_one_cannot_continue ... ok
test pipeline::boundary_tests::the_model_is_released_when_writing_ends_either_way ... ok
test pipeline::translate_tests::only_a_remake_that_failed_translating_continues ... ok
test pipeline::worker_tests::a_bug_fails_the_job_instead_of_leaving_it_running ... ok
test pipeline::worker_tests::every_ten_percent_step_passed_is_reported_once ... ok
test pipeline::translate_tests::an_older_builds_selection_never_pairs_a_transcript_with_another_ones_mishearings ... ok
test pipeline::worker_tests::a_job_in_an_unselected_account_resumes_in_that_database_and_publishes_its_completed_result ... ok
test sentences::tests::a_long_silence_ends_a_sentence_without_punctuation ... ok
test sentences::tests::a_segment_without_word_times_is_spread_over_its_time ... ok
test sentences::tests::a_sentence_ends_where_its_punctuation_ends_it ... ok
test sentences::tests::a_sentence_over_the_cap_is_cut_at_a_comma_then_a_conjunction_then_the_longest_pause ... ok
test sentences::tests::a_sentence_starting_mid_segment_starts_with_its_first_word ... ok
test sentences::tests::a_sentence_with_commas_goes_whole_however_long_and_only_a_bare_stretch_ends_at_the_limit ... ok
test sentences::tests::a_stretch_without_punctuation_is_cut_at_pauses_and_the_cap ... ok
test sentences::tests::cues_keep_after_the_previous_one ... ok
test sentences::tests::each_cue_is_one_sentence_and_none_holds_the_end_of_one_and_the_start_of_the_next ... ok
test sentences::tests::no_cue_ends_after_its_words_even_when_they_take_no_time ... ok
test sentences::tests::the_grouping_rebuilt_from_saved_cues_is_the_one_the_cues_were_made_with ... ok
test sentences::tests::the_open_sentence_waits_for_the_next_window_and_ends_the_same ... ok
test stitch::tests::a_loop_on_a_sound_is_cut_and_said_repetition_and_numbers_stay ... ok
test stitch::tests::a_side_that_heard_nothing_in_the_overlap_gives_way_to_the_other ... ok
test stitch::tests::a_word_cut_by_the_window_edge_appears_once_and_the_split_sentence_is_one_cue ... ok
test stitch::tests::natural_repetition_across_the_cut_keeps_every_spoken_repeat ... ok
test stitch::tests::pieces_move_to_the_source_timeline_inside_their_window ... ok
test stitch::tests::segments_without_words_are_reconciled_as_whole_units ... ok
test stitch::tests::without_a_common_word_the_overlap_is_shared_at_its_middle ... ok
test pipeline::worker_tests::a_job_resumed_before_its_notice_went_out_notifies_once ... ok
test pipeline::translate_tests::cues_are_translated_one_per_request_with_requests_overlapping ... ok
test pipeline::worker_tests::retranslation_resumes_partial_work_and_publishes_only_a_complete_set ... ok
test pipeline::boundary_tests::a_write_killed_with_a_sentence_open_loses_no_word_when_the_released_build_resumes_it ... ok
test store::tests::a_late_sign_in_does_not_replace_the_newer_opened_url ... ok
test store::tests::a_job_from_a_file_without_plans_keeps_plan_one_and_new_jobs_get_the_current_plan ... ok
test store::library_tests::legacy_completed_rows_keep_timing_and_deletion_removes_all_owned_rows ... ok
test store::tests::opening_a_current_file_does_not_wait_for_a_writer ... ok
test store::wal_tests::reads_do_not_wait_for_an_open_write ... ok
test store::tests::region_save_is_atomic_and_percent_never_drops ... ok
test pipeline::worker_tests::transient_retries_are_independent_for_glossary_and_each_cue ... ok
test pipeline::translate_tests::cost_is_metered_per_call_and_never_stops_translation ... ok
test sync::library::tests::active_jobs_defer_projection_and_confirmed_deletion_stays_hidden_until_cleanup ... ok
test store::library_tests::deletion_only_clears_the_browser_url_it_inspected ... ok
test sync::library::tests::concurrent_order_ties_have_the_same_visible_order_on_both_replicas ... ok
test sync::library::tests::failed_projection_rolls_back_received_records_and_cursor_together ... ok
test sync::library::tests::concurrent_completed_versions_are_retained_and_fresh_restoration_waits_for_cleanup ... ok
test sync::library::tests::invalid_legacy_completion_rolls_back_the_copy_and_active_jobs_stay_local ... ok
test sync::library::tests::job_completion_cannot_adopt_a_deletion_received_after_the_job_started ... ok
test sync::library::tests::multi_place_moves_reach_the_peer_in_the_same_order ... ok
test sync::library::tests::native_completion_and_its_journal_commit_atomically_and_do_not_replace_the_local_job ... ok
test sync::library::tests::deletion_and_a_fresh_link_or_translation_in_one_page_cannot_bypass_cleanup ... ok
test sync::library::tests::legacy_import_is_a_copy_is_idempotent_and_cannot_assign_another_accounts_store ... ok
test sync::library::tests::silent_cue_repair_denies_any_change_to_the_inspected_selection_or_cue ... ok
test sync::library::tests::queued_remote_deletion_invalidates_earlier_consent_before_projection ... ok
test sync::library::tests::ordinary_library_mutations_publish_and_incoming_projection_does_not_echo ... ok
test sync::library::tests::partial_dependencies_survive_restart_and_blocked_rows_do_not_starve_later_work ... ok
test sync::library::tests::received_ready_library_is_usable_without_processing_and_survives_restart ... ok
test sync_channel::tests::different_signed_google_subjects_with_the_same_email_cannot_transfer_any_library_data ... ok
test sync::library::tests::silent_cue_repair_is_atomic_retains_versions_cost_and_sync_selection ... ok
test pipeline::worker_tests::failed_retranslation_preserves_completed_subtitles_for_every_stop ... ok
test result: ok. 87 passed; 0 failed; 0 ignored; 0 measured; 173 filtered out; finished in 177.48s
```

The whole library suite (`cargo test -q --lib`) at the same commit printed
`258 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 174.52s`.

## Isolated API checks (contract tests)

The contract host is this commit's `vto-contract-host`, built with
`cargo build -q -p video-translate-overlay --bin vto-contract-host` at
`5c76696` with a clean tree and passed as
`VTO_CONTRACT_HOST=C:/Users/dasdk/PycharmProjects/video-translate-overlay-2/target/debug/vto-contract-host.exe`:

- 25,945,600 bytes, written 2026-11-19 15:50:08 +0900;
- SHA-256 `eeb924ef4ccb9538de5545d06d912eb9f818c893191256785c2e3fc4570fce6c`.

The harness (`contract-tests/src/lib.rs`) gives each test its own folder,
`%TEMP%\vto-contract-<pid>-<nanos>-<n>`, for the host's data, keystore and
notifications, and starts its fake Gemini and media server on loopback ports
the system picks (`127.0.0.1:0`). The host's config points it there, so
nothing reaches the network or the account library. `a_split_sentence_goes_to_gemini_as_one_request_of_its_parts`
also checks that `GET /videos/{key}/subtitles` returns cues of exactly
`end_ms, seq, start_ms, text_ko`. Command:
`cargo test -p vto-contract-tests --test <file>`. The results:

```
sentences: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.53s
translation: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 25.47s
write_and_timing: test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.80s
library: test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.17s
jobs: test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.63s
```

A second run of the first three files, with the same host, printed each
test's name:

```
== --test sentences
test fillers_stay_in_the_saved_source_and_the_instruction_leaves_them_out_of_the_korean ... ok
test a_split_sentence_goes_to_gemini_as_one_request_of_its_parts ... ok
test a_retranslation_asks_for_each_sentence_as_the_first_translation_did ... ok
test mishearings_found_with_the_glossary_are_stored_and_carried_in_every_request ... ok
test a_reply_with_a_missing_extra_or_duplicate_part_is_not_saved ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.94s
== --test translation
test retranslation_keeps_saved_source_times_and_the_cost_already_spent ... ok
test test_translation_makes_korean_cues ... ok
test test_cost_never_stops_translation ... ok
test test_terms_stay_consistent_across_batches ... ok
test test_one_hour_lecture_costs_under_budget ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 27.81s
== --test write_and_timing
test test_silent_span_has_no_cue ... ok
test test_cue_times_match_source_after_silence_trim ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.49s
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

```
== npx vitest run -c vitest.ui.config.ts tests/ui/subtitles.test.svelte.ts --reporter=verbose
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > local subtitles scale with their player and wrap inside a letterboxed image 246ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > a long Korean cue renders on two balanced lines at phone, windowed and fullscreen sizes 70ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deleting another video preserves cues through a failed refresh: native ready 1007ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deleting another video preserves cues through a failed refresh: native failed 1062ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deleting another video preserves cues through a failed refresh: broadcast ready 1011ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deleting another video preserves cues through a failed refresh: broadcast failed 1015ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > a late subtitle response after deletion cannot restore cached cues: native ready 186ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > a late subtitle response after deletion cannot restore cached cues: native failed 187ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > a late subtitle response after deletion cannot restore cached cues: broadcast ready 188ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > a late subtitle response after deletion cannot restore cached cues: broadcast failed 172ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deletion before the first job response cannot revive that video: native 1183ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deletion before the first job response cannot revive that video: broadcast 1184ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > confirmed recreation replaces earlier deletion markers: native known identity 189ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > confirmed recreation replaces earlier deletion markers: native unknown identity 251ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > confirmed recreation replaces earlier deletion markers: broadcast known identity 186ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > confirmed recreation replaces earlier deletion markers: broadcast unknown identity 251ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deletion after consent still rejects a late creation response: native 187ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > deletion after consent still rejects a late creation response: broadcast 188ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > the cue under the playback time, hidden while an ad plays 64ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > failed jobs recover saved subtitles and keep the failure reason: initial, saved=true 62ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > failed jobs recover saved subtitles and keep the failure reason: initial, saved=false 62ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > failed jobs recover saved subtitles and keep the failure reason: poll, saved=true 1059ms
 ✓ |chromium| tests/ui/subtitles.test.svelte.ts > failed jobs recover saved subtitles and keep the failure reason: poll, saved=false 1062ms
 Test Files  1 passed (1)
      Tests  23 passed (23)
== npx vitest run tests/overlay/placement.test.ts --reporter=verbose
 ✓ |chromium| tests/overlay/placement.test.ts > test_overlay_lies_over_the_video_and_follows_it_into_full_screen 73ms
 ✓ |chromium| tests/overlay/placement.test.ts > a long Korean cue renders on two balanced lines at phone, windowed and fullscreen sizes 26ms
 ✓ |chromium| tests/overlay/placement.test.ts > test_a_video_too_small_to_hold_it_puts_the_overlay_at_the_bottom 6ms
 ✓ |chromium| tests/overlay/placement.test.ts > ready subtitles stay inside a landscape fullscreen player and survive returning to portrait 16ms
 ✓ |chromium| tests/overlay/placement.test.ts > the fullscreen button requests the player with its saved subtitles, then exits 2ms
 Test Files  1 passed (1)
      Tests  5 passed (5)
== npx vitest run -c docs/evidence/sentence-cues/measure/measure.config.ts --reporter=verbose
 ✓ |chromium| docs/evidence/sentence-cues/measure/measure.test.svelte.ts > page overlay 5299ms
 ✓ |chromium| docs/evidence/sentence-cues/measure/measure.test.svelte.ts > watch screen 4269ms
 Test Files  1 passed (1)
      Tests  2 passed (2)
```

The files the run wrote and read, by SHA-256:

| File | SHA-256 |
| --- | --- |
| `docs/evidence/sentence-cues/measure/expected.json` (committed; asserted equal) | `1eceedeed6dc700ca89e5a19d5b9ece340996da5d24a9db88e14e4aec9448905` |
| `artifacts/sentence-cues/measured-overlay.json` (written by the run) | `d7374ad1f0548be4e0607e6ad8763b48fcb226a4997a6fc1404b92300ee5dce9` |
| `artifacts/sentence-cues/measured-watch.json` (written by the run) | `c3adb167ee568013bc4636a2830b69b86d03580396bf666e30f73f3b400f1949` |
| `artifacts/sentence-cues/ko.json` (input, not committed) | `f6ad086f05c8522d8f9ae89689cd427014da8fd59eac290ec43a0be4f0ae16d0` |

`python -I -X utf8 docs/evidence/sentence-cues/measure/check_readme.py`
checks that the README's rendered-line tables are sums of `expected.json`
and that every exception it holds is listed. It printed `differences 0`.

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
`57f7173` and `5c76696` (`git diff --stat 57f7173 5c76696 -- src/ tests/`
prints nothing).

## The installed Windows app

This run is reused from the code at `57f7173`, by this task. `5c76696`
changes nothing the released app runs: the app is the installed 0.1.9 build,
and the library it opened is unchanged since. Re-checked on 2026-11-19
after `5c76696`: the exe's SHA-256 is still `7ff268da…ce0c`; the library file
is still 4,378,624 bytes, last written 12:10:18 +0900, SHA-256 `d4c1aded…7756`.
Its write-ahead log (238,992 bytes, last written 13:20:09 +0900) holds that
app session's writes. The app is not running. The unchanged subtitle shape
at `5c76696` is shown by the contract test above.

The installed app is version 0.1.9:

- path `%LOCALAPPDATA%\video-translate-overlay\video-translate-overlay.exe`;
- FileVersion and ProductVersion 0.1.9, 75,448,832 bytes, written 2026-11-18 03:16:30Z;
- SHA-256 `7FF268DA80DCF2C8F8E563F82BB4997A7B05CCFE575F85ACA63A0348A1EECE0C`.

The account library it opened is
`%LOCALAPPDATA%\dev.vto.overlay\sync\9d06ffd2…5d956\sync.sqlite3`. Before
launch it was 4,378,624 bytes with an empty write-ahead log, SHA-256
`d4c1adedf23a2dd5fe9ff790ba46d6e24a5b20953f335e50787da9471ca7756`, and
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
