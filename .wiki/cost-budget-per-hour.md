---
scope: project
severity: contract
triggers: ["비용", "요금", "가격", "토큰", "1000원", "천 ?원", "cost", "API ?(키|호출)"]
slots: []
sources: [README.md, docs/design/backend.md, src-tauri/README.md, contract-tests/tests/translation.rs]
links: [translate-text-not-audio, repository-gate, project]
---

# Translation aims at about 1,000 KRW per hour of video; every call is metered, and cost never stops a job

Rule. Translation cost is metered from the usage Gemini reports, multiplied
by the unit price and exchange rate held in `src-tauri/src/gemini.rs`
(assumption A6 in `docs/design/backend.md`), and added to the job's
`cost_krw`. A re-translation starts from what the video spent before. The
app does not stop a job, or refuse a re-translation, for cost: the person
removed that stop on 2026-11-19, calling a hard limit slop. `cost_limit`
appears only on jobs from before then. Two designed checks pin this:
- T9, `test_one_hour_lecture_costs_under_budget`: the reported cost equals
  usage times price and rate, and an hour stays under 1,000 KRW.
- T10, `test_cost_never_stops_translation`: with absurdly large usage the
  job still ends ready, and its cost is every call's usage times price and
  rate.

Both are in `contract-tests/tests/translation.rs` and run inside
[[repository-gate]]. Adding a paid service the meter does not count, or
bringing a stop back, is the user's decision. The 요약 button's summary call
(P19) is metered with the same price and rate into the `summary` table, valid
reply or not, and P7 adds it to the video's shown `cost_krw`; the job's own
`cost_krw`, which T9, T10 and a re-translation read, stays translation only.

Why. The original request (quoted in `docs/design/backend.md`) asks for
about 1,000 KRW per hour. The eight re-made lectures cost 290–410 KRW per
hour, so the target is kept by translating text only, one sentence per
request ([[translate-text-not-audio]]), not by a stop.

What goes wrong. A paid call the meter does not count makes the cost shown to
users too low. A request design that multiplies calls per sentence moves the
per-hour figure past the target with nothing to stop it, so T9 is the guard.
