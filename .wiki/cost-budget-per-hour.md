---
scope: project
severity: contract
triggers: ["비용", "요금", "가격", "토큰", "1000원", "천 ?원", "cost", "API ?(키|호출)"]
slots: []
sources: [README.md, docs/design/backend.md, src-tauri/README.md, contract-tests/tests/translation.rs]
links: [translate-text-not-audio, repository-gate, project]
---

# Translation cost stays under 1,000 KRW per hour of video, and the app stops itself above it

Rule. Translation cost is metered from the usage Gemini reports, multiplied
by the unit price and exchange rate held in `src-tauri/src/gemini.rs`
(assumption A6 in `docs/design/backend.md`). When a job passes 1,000 KRW per
hour of video, translation stops with `cost_limit`. Two designed checks pin
this:
- T9, `test_one_hour_lecture_costs_under_budget`: the reported cost equals
  usage times price and rate, and stays under the limit.
- T10, `test_cost_limit_stops_translation`: above the limit, Gemini is not
  called again and the job fails with `cost_limit`.

Both are in `contract-tests/tests/translation.rs` and run inside
[[repository-gate]]. Raising the limit, or adding a paid service the meter
does not count, is the user's decision. The budget is their stated
requirement.

Why. The original request (quoted in `docs/design/backend.md`) asks for
about 1,000 KRW per hour, and `README.md` states the same figure to users.

What goes wrong. A paid call the meter does not count bypasses the stop, so
the per-hour figure users are promised no longer holds.
