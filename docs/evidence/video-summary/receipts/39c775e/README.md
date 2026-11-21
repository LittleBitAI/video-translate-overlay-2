# Receipts at 39c775e19a086a3210ac8912484566ec666b0d98

Made after the commit they describe, by the commands below. The commit that adds this folder changes nothing outside `docs/evidence/video-summary/`. To check that the head under review runs the code tested here, compare each tree id below with `git rev-parse <reviewed head>:<path>`; every path except `docs` must match.

| Path | Tree or blob id at the tested head |
| --- | --- |
| `.cargo` | `a1a23fdf27299b9c0c4cbc52f0f75b28effc6791` |
| `.claude` | `1ebb894b3542033bd4bb52c04c1228c27cf49e35` |
| `.config` | `7b75d9a30ae480bf8efaae049b16293ec72d3bb3` |
| `.gitattributes` | `95c1951cd30655419d3fc2cd7ee0e600ab42e93c` |
| `.github` | `e6181cb1e20b3bfb83683a3290c7423965892d6e` |
| `.gitignore` | `89b2c91ef5b014955ea6d8413f9c317e2dd86228` |
| `.wiki` | `e091b2c99f230b3c3b1b642858fe4e7118bf88d2` |
| `Cargo.lock` | `e7dcae50a0598fd7061e6e9942303f0046e6f52e` |
| `Cargo.toml` | `8312d4eed80cdf2fa51b371225ff30bd002e2a95` |
| `DESIGN.md` | `50fabff531950caa34a36d47affc6f7e47611298` |
| `LICENSE` | `57bc88a15a0ee8266c259b2667e64608d3f7e292` |
| `README.md` | `b325a77e37e40bb90e31258683e486db9141c74b` |
| `contract-tests` | `028e2760b8928ffd96933798f170192ddaa07711` |
| `docs` | `c05693985be63677976aba54dd4015d8ccbb5a36` |
| `e2e` | `cf48272380dd7aadb9dc99f9b391c62f758837bb` |
| `index.html` | `6051cc7e4edd98aa56ff67691ff908054e30389d` |
| `overview.html` | `ab5bfbfca207ffbe5b05c77a8767e85f4a058b2b` |
| `overview` | `b9d9141789e0cdda0c10ef4b0f2ecf6c9e03e008` |
| `package-lock.json` | `3457976ced531f9af45403a396e460c480013337` |
| `package.json` | `36a0a7491da1606ec9ad5dd7366786672c8ff7a2` |
| `scripts` | `3f3b6241873dfe8984a3a4ab69209a7b07f8c595` |
| `src-tauri` | `beae85b463510a38ba33bee448dddf643df97623` |
| `src` | `ddb80385123e3ebd6bb02563a78fb56e635f11f0` |
| `tests` | `e42d0dbfe2c16c4b64a919e615f7e03385dfe988` |
| `verification.json` | `3aa7f194e9b097cfb6fbbf8c3e0b1cd755c48b97` |
| `vite.config.ts` | `68c8020a02cf7b245e1c107865a43ccf83e39ab1` |
| `vite.overlay.config.ts` | `d81ed740eaec3f80d99739ee6c9de47f1653e027` |
| `vitest.config.ts` | `dadf10a9f608ab771ab6b7f77295802d112cf7ec` |
| `vitest.ui.config.ts` | `ddab02f7244ff248ddda23770a8be57df2d8d1cb` |

```text
head: 39c775e19a086a3210ac8912484566ec666b0d98
rustc 1.98.1 (48a229cea 2026-09-01)
cargo 1.98.1 (797e8a9bc 2026-08-05)
node v24.18.0

$ cd src-tauri && cargo test --lib summary::
test summary::tests::checked_values_drop_blanks_and_repeats ... ok
test summary::tests::each_field_is_held_to_its_language ... ok
test summary::tests::only_the_transcript_decides_the_talk_s_scripts ... ok
test summary::tests::the_same_json_always_renders_the_same_markdown ... ok
test summary::tests::values_that_break_a_rule_are_refused ... ok
test summary::flow_tests::signing_out_during_the_call_keeps_the_reply_and_its_usage_in_the_account_that_paid ... ok
test summary::flow_tests::a_video_marked_for_deletion_is_never_sent_to_gemini ... ok
test summary::flow_tests::a_read_during_and_after_a_call_tells_what_the_call_did ... ok
test summary::flow_tests::a_press_queued_behind_another_call_says_pending ... ok
test summary::flow_tests::a_summary_is_stored_only_into_the_entry_it_was_read_from ... ok
test summary::flow_tests::a_queued_press_spends_only_on_the_entry_it_was_made_on ... ok
test summary::flow_tests::a_read_at_either_edge_of_a_call_says_pending ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 277 filtered out; finished in 35.98s

$ cd src-tauri && cargo test --lib a_summary_counts_only
test store::library_tests::a_summary_counts_only_for_the_video_row_it_was_made_for ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 288 filtered out; finished in 7.24s

$ cargo build -q --workspace --bin vto-contract-host && VTO_CONTRACT_HOST=target/debug/vto-contract-host.exe cargo test -p vto-contract-tests --test summary --test translation
     Running tests\summary.rs (target\debug\deps\summary-e7ca94ff57b06e44.exe)
test test_only_a_ready_video_on_the_app_screen_can_be_summarized ... ok
test test_summary_request_carries_title_source_transcript_terms_and_schema ... ok
test test_a_reply_that_breaks_a_value_rule_is_not_stored_but_is_metered ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.30s
     Running tests\translation.rs (target\debug\deps\translation-d33525ad9bc48df6.exe)
test test_translation_makes_korean_cues ... ok
test retranslation_keeps_saved_source_times_and_the_cost_already_spent ... ok
test test_cost_never_stops_translation ... ok
test test_terms_stay_consistent_across_batches ... ok
test test_one_hour_lecture_costs_under_budget ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 36.83s

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
✓ built in 1.16s

$ PORT=8793 APP_DATA_DIR=<empty dir> node scripts/live.mjs --stand-ins   # GET /build: {"head":"39c775e19a086a3210ac8912484566ec666b0d98","pid":27064}
$ node scripts/verification/video-summary.mjs http://127.0.0.1:8793 <out>
21/21 checks passed at head 39c775e19a086a3210ac8912484566ec666b0d98
report.json sha256 a85a3430b01ba39b7318f8c711c38a216477055a71aa4bca172953f27f190047
```
