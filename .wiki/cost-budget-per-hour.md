---
scope: project
severity: contract
triggers: ["비용", "요금", "가격", "토큰", "1000원", "천 ?원", "cost", "API ?(키|호출)"]
slots: []
sources: [README.md]
links: [translate-text-not-audio, project]
---

# Every model or API choice is costed against about 1,000 KRW per hour of video

Rule. When a change adds or swaps a paid call — a translation model, a
hosted speech-to-text service, a voice-conversion API — state its estimated
cost per hour of source video next to the change. If the estimate goes above
roughly 1,000 KRW per hour, ask the user before adopting it.

Why. `README.md` sets the target at about 1,000 KRW per hour and gives cost
as the reason for the whole pipeline shape.

What goes wrong. A choice that is cheap per call but billed per audio second
or per output token quietly multiplies the hourly cost, and nobody sees it
until the bill.
