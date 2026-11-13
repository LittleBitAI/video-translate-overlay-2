---
scope: project
severity: contract
triggers: ["번역", "gemini", "제미나이", "STT", "음성 ?인식", "받아쓰", "전사", "자막 ?생성"]
slots: []
sources: [README.md, docs/design/backend.md, contract-tests/tests/translation.rs]
links: [cost-budget-per-hour, subtitles-must-not-drift, project]
---

# Transcribe locally, then translate the text with gemini-3.1-flash-lite — never send audio to Gemini for Korean subtitles

Rule. The pipeline is two stages. First whisper.cpp writes the downloaded
audio out as foreign-language text on the device (assumption A4 in
`docs/design/backend.md`). Then `gemini-3.1-flash-lite` translates that text
into Korean. T4, `test_translation_makes_korean_cues` in
`contract-tests/tests/translation.rs`, pins the model name. Replacing the
pair with Gemini's direct audio-to-Korean subtitling, or moving the default
model to a larger Gemini tier, is the user's decision.

Why. The original request (quoted in `docs/design/backend.md`) says the
direct Gemini audio-to-Korean path exists but is too expensive. The
two-stage design exists to stay near 1,000 KRW per hour while keeping
translation quality.

What goes wrong. The app produces subtitles that look right but cost many
times the budget, which defeats the product's stated purpose.
