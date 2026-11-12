---
scope: project
severity: contract
triggers: ["번역", "gemini", "제미나이", "STT", "음성 ?인식", "받아쓰", "전사", "자막 ?생성"]
slots: []
sources: [README.md]
links: [cost-budget-per-hour, subtitles-must-not-drift, project]
---

# Transcribe locally, then translate the text with gemini-3.1-flash-lite — never send audio to Gemini for Korean subtitles

Rule. The pipeline is two stages. A model the app runs itself turns the
downloaded audio into foreign-language text; `gemini-3.1-flash-lite` then
translates that text into Korean. Do not replace the pair with Gemini's
direct audio-to-Korean subtitling, and do not switch the default translation
model to a larger Gemini tier, without the user's decision.

Why. `README.md` states that the direct Gemini audio-to-Korean path exists
but is too expensive, and that the whole reason for the two-stage design is
to stay near 1,000 KRW per hour while keeping translation quality.

What goes wrong. The app produces subtitles that look right but cost many
times the budget, which defeats the product's stated purpose.
