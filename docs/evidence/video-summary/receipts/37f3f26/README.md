# Focused receipts at 37f3f269cf508de34f54e54a5b7741052d500155

Made after the commit they describe; the commit that adds this folder changes only evidence files.

```text
head: 37f3f269cf508de34f54e54a5b7741052d500155
rustc 1.98.1 (48a229cea 2026-09-01)
cargo 1.98.1 (797e8a9bc 2026-08-05)
v24.18.0

$ cd src-tauri && cargo test --lib summary::
test summary::tests::checked_values_drop_blanks_and_repeats ... ok
test summary::tests::the_same_json_always_renders_the_same_markdown ... ok
test summary::tests::values_that_break_a_rule_are_refused ... ok
test summary::tests::each_field_is_held_to_its_language ... ok
test summary::flow_tests::signing_out_during_the_call_keeps_the_reply_and_its_usage_in_the_account_that_paid ... ok
test summary::flow_tests::a_video_marked_for_deletion_is_never_sent_to_gemini ... ok
test summary::flow_tests::a_read_during_and_after_a_call_tells_what_the_call_did ... ok
test summary::flow_tests::a_press_queued_behind_another_call_says_pending ... ok
test summary::flow_tests::a_summary_is_stored_only_into_the_entry_it_was_read_from ... ok
test summary::flow_tests::a_read_at_either_edge_of_a_call_says_pending ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 277 filtered out; finished in 9.39s

$ cd src-tauri && cargo test --lib a_summary_counts_only
test store::library_tests::a_summary_counts_only_for_the_video_row_it_was_made_for ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 286 filtered out; finished in 3.35s

$ cargo build -q --workspace --bin vto-contract-host && VTO_CONTRACT_HOST=target/debug/vto-contract-host.exe cargo test -p vto-contract-tests --test summary --test translation
     Running tests\summary.rs (target\debug\deps\summary-e7ca94ff57b06e44.exe)
test test_summary_request_carries_title_source_transcript_terms_and_schema ... ok
test test_only_a_ready_video_on_the_app_screen_can_be_summarized ... ok
test test_a_reply_that_breaks_a_value_rule_is_not_stored_but_is_metered ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.76s
     Running tests\translation.rs (target\debug\deps\translation-d33525ad9bc48df6.exe)
test retranslation_keeps_saved_source_times_and_the_cost_already_spent ... ok
test test_translation_makes_korean_cues ... ok
test test_cost_never_stops_translation ... ok
test test_terms_stay_consistent_across_batches ... ok
test test_one_hour_lecture_costs_under_budget ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.72s

$ npx vitest run --config vitest.ui.config.ts tests/ui/summary.test.ts --reporter=verbose
 ✓ |chromium| tests/ui/summary.test.ts > the Markdown is read as headings, lists and paragraphs of text, unescaped, never as HTML
 ✓ |chromium| tests/ui/summary.test.ts > a ready video offers 요약; pressing it shows the loading state, then the rendered summary and its cost
 ✓ |chromium| tests/ui/summary.test.ts > a stored summary is shown on opening, with no POST
 ✓ |chromium| tests/ui/summary.test.ts > a translating video does not offer 요약
 ✓ |chromium| tests/ui/summary.test.ts > a failed video does not offer 요약
 ✓ |chromium| tests/ui/summary.test.ts > an untranslated link (no job) does not offer 요약
 ✓ |chromium| tests/ui/summary.test.ts > a reply that breaks a rule shows the error with 다시 시도, which asks again
 ✓ |chromium| tests/ui/summary.test.ts > a refused 다시 요약 keeps the summary and shows its cost with the billed reply added
 ✓ |chromium| tests/ui/summary.test.ts > a reply that arrives after the screen was left is dropped there and shown when the video is opened again
 ✓ |chromium| tests/ui/summary.test.ts > reopened during a first summary: waits, then shows what the call did
 ✓ |chromium| tests/ui/summary.test.ts > reopened during a first summary whose reply breaks a ru…: waits, then shows what the call did
 ✓ |chromium| tests/ui/summary.test.ts > reopened during a re-summary: waits, then shows what the call did
 ✓ |chromium| tests/ui/summary.test.ts > reopened during a re-summary whose reply breaks a rule: waits, then shows what the call did
 ✓ |chromium| tests/ui/summary.test.ts > without a key, the error says to add one in the settings
 Test Files  1 passed (1)
      Tests  14 passed (14)

$ npm run build
✓ built in 1.43s

$ PORT=8793 APP_DATA_DIR=<empty dir> node scripts/live.mjs --stand-ins   # GET /build: {"head":"37f3f269cf508de34f54e54a5b7741052d500155","pid":18980}
$ node scripts/verification/video-summary.mjs http://127.0.0.1:8793 <out>
21/21 checks passed at head 37f3f269cf508de34f54e54a5b7741052d500155
report.json sha256 0beb27e25b723c0cd8b7aa7636873a3c07d3f18de98be5d38f6b251257118bf3
```
