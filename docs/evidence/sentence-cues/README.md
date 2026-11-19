# Sentence cues and speech cleanup: the eight re-made videos

This records how the eight saved YouTube videos were made again with sentence
cues, what that cost, what changed in their subtitles, and my own reading of
old against new Korean. Everything here was measured after the last re-make
(2026-11-19), on the account library the app uses.

## What changed in the pipeline

- Cues are rebuilt by sentence from the word times written by the speech
  model. A sentence longer than two subtitle lines (70 English characters) is
  split at a comma, before a conjunction, or at the longest pause, into parts.
  A stretch without punctuation is ended at pauses and the cap (a forced end).
- One request carries one whole sentence: all its parts, numbered. The Korean
  comes back in the same parts and is saved only if the reply has exactly
  those part ids (A6 in `docs/design/backend.md`).
- Fillers, false starts, immediate repeats and tics are left out of the
  Korean. The saved English keeps them as heard.
- The glossary request also returns suspected mishearings, carried in every
  translation request.
- PC decoding gets the video title as a hint (see below).
- Cost no longer stops translation (decided by the person on 2026-11-19). It
  is still metered per call and recorded on top of the video's earlier
  spending.

## The re-makes (done items 12 and 13)

The account library was copied before the first re-make
(`artifacts/sentence-cues/backup/account-before-remake.sqlite3`, SHA-256
`0d60012d86b9d1fe1d37c80d525e31331097159f40429b071ee7ca84633d748c`). The
backups stay out of Git because they hold the person's library. Each video was
made again from its audio by `src-tauri/examples/remake_saved.rs` (PC speech
model, this build's pipeline), as a new saved version. The new version is
selected, and the old one stays in the saved-version selector. After the last
re-make, a check of every video's records found:

- the new version selected, and the cue table equal to it;
- the version selected before the task still present;
- each new version's `cost_krw` equal to the video's earlier spending plus this re-make's;
- `PRAGMA integrity_check` ok.

The installed 0.1.9 Library shows "저장본 2개" or "저장본 3개" on every card.

| Video | Spent before | Paid now | Recorded after | 1,000 KRW per hour of video |
| --- | ---: | ---: | ---: | ---: |
| `QDX-1M5Nj7s` (58:12) | 276.29 | 296.70 | 572.99 | 970.0 |
| `QWNxQIq0hMo` (15:16), two re-makes | 52.70 | 76.27 + 75.70 | 204.67 | 254.4 |
| `WJpeW9lCSUM` (23:14) | 122.99 | 139.25 | 262.24 | 387.2 |
| `WjhDDeZ7DvM` (39:57) | 143.56 | 199.18 | 342.74 | 665.8 |
| `_MEqhFwnp8o` (49:51) | 203.63 | 249.09 | 452.72 | 830.8 |
| `vSCYL8ySh9g` (15:04), two re-makes | 90.28 | 73.96 + 73.86 | 238.10 | 251.1 |
| `wbc-oxFFV9c` (1:16:12) | 462.01 | 502.81 + 0.99 + 1.02 | 966.84 | 1,270.0 |
| `zjkBMFhNj_g` (59:48) | 373.09 | 408.01 | 781.10 | 996.7 |

All amounts are in KRW. Paid Gemini spending for the task was 2,152.51 KRW:
55.66 KRW of trials on short excerpts and 2,096.85 KRW of re-makes. Every
video stayed under 1,000 KRW per hour of video, including what it had spent
before. The re-makes cost 290 to 410 KRW per hour.

- Dory and the 15-minute DeepSeek video were made a second time, as the
  person approved, after the run-on fix (`e3eec07`). The first re-make of
  each stays as a saved version.
- wbc's first run ended `translate_failed` with one cue untranslated (cue
  1388, 70:46). Gemini answered that one-part sentence with two segments
  (ids 1388 and an invented 1389), which the id check refused. The tool's
  rollback had done nothing, because selecting the version that is already
  selected keeps the cue table. That was fixed (`2c78179`). The job was then
  continued twice for the one cue; the second reply matched. A restore between
  the two left a newer job row for the old version, so the installed app showed
  the old cost. The tool now makes a continued re-make the newest job, and
  that row was repaired in the library.
- `WJpeW9lCSUM`'s first 34 writing windows were made before the run-on fix;
  its stray cues below come from that part.

## Cue shape (done item 14)

Counted on the cue table of every video, old (before the task) → new. The old
totals reproduce the spec's baseline. The counts are made by
`artifacts/sentence-cues/metrics.py`.

- Not ending: the English cue does not end with `.`, `?` or `!`.
- Parts / forced ends:
  - parts are cues that continue a sentence translated whole in one request;
  - forced ends are sentence ends the cue builder placed where the speech had none.
- Mid-start: the English cue starts with a lower-case letter.
- Two ends: the cue holds two or more sentence ends.
- KO > 60: Korean over 60 characters (the rendered line counts are below).
- Fillers and repeats in English: `uh`/`um` and immediate repeats in the saved English, which keeps what was heard.
- Fillers and repeats in Korean: the same in the Korean.

| Video | Cues | Not ending | Parts / forced ends (new) | Mid-start | Two ends | KO > 60 | EN fillers | EN repeats | KO fillers | KO repeats |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `QDX-1M5Nj7s` MIT 6.S191 lecture (58 min) | 754 → 1,316 | 285 → 886 | 803 / 86 | 288 → 880 | 48 → 3 | 92 → 0 | 0 → 1 | 10 → 16 | 3 → 0 | 1 → 0 |
| `QWNxQIq0hMo` Build DeepSeek: Series Introduction (15 min) | 165 → 313 | 70 → 183 | 171 / 12 | 71 → 162 | 7 → 1 | 39 → 1 | 1 → 0 | 3 → 2 | 3 → 0 | 0 → 0 |
| `WJpeW9lCSUM` Nemotron Bolt 2 Bit (23 min) | 309 → 499 | 175 → 269 | 259 / 10 | 171 → 256 | 2 → 0 | 41 → 0 | 0 → 0 | 1 → 1 | 3 → 0 | 0 → 0 |
| `WjhDDeZ7DvM` Build DeepSeek: Series Introduction (40 min) | 408 → 770 | 167 → 458 | 426 / 32 | 164 → 434 | 14 → 0 | 86 → 2 | 2 → 3 | 4 → 2 | 3 → 0 | 1 → 0 |
| `_MEqhFwnp8o` Ask the Experts: Dory (50 min) | 678 → 1,008 | 398 → 485 | 444 / 40 | 394 → 453 | 17 → 0 | 78 → 0 | 139 → 7 | 50 → 10 | 60 → 0 | 4 → 0 |
| `vSCYL8ySh9g` Dory - looped transformer (15 min) | 202 → 314 | 106 → 188 | 182 / 6 | 103 → 176 | 7 → 0 | 22 → 0 | 0 → 0 | 3 → 3 | 0 → 0 | 0 → 0 |
| `wbc-oxFFV9c` Give Me 1 Hour, Deep Learning (76 min) | 885 → 1,490 | 307 → 540 | 536 / 8 | 311 → 520 | 91 → 1 | 202 → 2 | 0 → 0 | 5 → 3 | 1 → 1 | 1 → 1 |
| `zjkBMFhNj_g` Intro to Large Language Models (60 min) | 892 → 1,409 | 375 → 781 | 740 / 41 | 366 → 745 | 53 → 0 | 97 → 0 | 8 → 1 | 3 → 3 | 17 → 0 | 0 → 1 |
| All 8 | 4,293 → 7,119 | 1,883 → 3,790 | 3,561 / 235 | 1,868 → 3,626 | 239 → 5 | 657 → 5 | 150 → 12 | 79 → 40 | 90 → 1 | 7 → 2 |

Reading the table:

- Most cues that do not end at a sentence end, or start mid-sentence, are
  now parts of a split sentence. 3,561 of the 3,790 are such parts.
  - Their sentence was translated in one request, so the Korean of each part
    is in Korean order.
  - In the old version, every such cut was translated alone.
  - The rest, 235, are forced ends in unpunctuated stretches; QDX has 86 of
    them.
- Cues holding two sentences fell from 239 to 5. None of the 5 holds two real
  sentences:
  - three are periods the speech model wrote before a lower-case word (QDX
    6:09, 13:52, 35:40);
  - two are abbreviations ("Dr.", "U.S.").
- The English fillers and repeats fell because the new transcripts write fewer
  of them, not because they were removed. The saved English is kept as heard.
- In the Korean, fillers fell from 90 to 1, and that one is a false match on
  `'cat'에` (wbc 27:28). Repeats fell from 7 to 2:
  - "0, 0, 0, 그리고 1" (wbc 5:20) is real content;
  - "덩어리" ×7 (zjk 36:55) is a regression, listed below.

## Rendered lines (done item 10)

Every Korean cue of the eight videos, old and new, was rendered in the page
overlay and the Watch screen at the three sizes, in Chromium, with the
players the UI line tests use. The line counts come from the same measurement
as `tests/subtitle-lines`. Each cell gives cues on 1 line / 2 lines / more
than 2 lines.

| Surface | Size | Old | New |
| --- | --- | --- | --- |
| Page overlay | windowed PC 1280×800 | 2,064 / 2,092 / 137 (up to 7) | 6,834 / 285 / 0 |
| Page overlay | fullscreen PC 1920×1080 | 2,098 / 2,061 / 134 (up to 7) | 6,854 / 265 / 0 |
| Page overlay | phone 384×854 | 1,261 / 2,396 / 636 (up to 9) | 5,580 / 1,539 / 0 |
| Watch screen | windowed PC 1280×800 | 2,046 / 2,105 / 142 (up to 7) | 6,823 / 296 / 0 |
| Watch screen | fullscreen PC 1920×1080 | 2,089 / 2,070 / 134 (up to 7) | 6,846 / 273 / 0 |
| Watch screen | phone 384×854 | 997 / 2,210 / 1,086 (up to 10) | 4,803 / 2,306 / 10 |

Exceptions: 10 new cues take three lines in the Watch screen at phone size,
and in no other place. Its player is 352 px wide, and its subtitle font fits
about 20 Korean characters a line. These 10 cues have 47 to 60 Korean
characters with few break points. The 70-character English cap does not bound
the Korean, which is sometimes longer than the cap assumed.

- `QDX-1M5Nj7s` 1271
- `QWNxQIq0hMo` 87, 221
- `WJpeW9lCSUM` 143
- `_MEqhFwnp8o` 86, 906 (an e-mail address)
- `wbc-oxFFV9c` 227, 484, 1317
- `zjkBMFhNj_g` 1224

In the page overlay at phone size, these cues fit two lines.

## The installed app (done item 16)

The installed Windows app is version 0.1.9. It opened `wbc-oxFFV9c` from its
Library. Its overlay showed:

- the new first cue ("1시간만 투자하세요, …"), with "자막 준비됨 · 966.8원";
- during playback, the new cue at 16:59 ("정확히 얼마나 적을까요?"; the old one was "얼마나 적을까요?").

From the video window, `get_subtitles` returned 1,490 cues with
`video_key`, `duration_ms`, `source_language`, `account_generation` and
`cues`. Each cue is `{seq, start_ms, end_ms, text_ko}`, the shape the old
version returned. Cue 1388 is present. The contract test
`a_split_sentence_goes_to_gemini_as_one_request_of_its_parts`
(`contract-tests/tests/sentences.rs`) checks the same cue keys on every build.

## The title hint on PC (done item 8)

PC decoding now gets the video title as whisper's initial prompt (the phone
does not). Same build (`transcribe_timing`, release, Vulkan,
large-v3-turbo q5_0), same audio, with and without `--hint <title>`. Word
changes were counted by `artifacts/sentence-cues/hintdiff.py` (a word
diff of the two transcripts, case and punctuation ignored).

| Video | Words without / with | Changed spans | Sentence ends without / with | Words in unpunctuated runs over 40 words, without / with |
| --- | --- | --- | --- | --- |
| `vSCYL8ySh9g` Dory (15 min) | 2,822 / 2,796 | 38 | 113 / 126 | 794 / 365 |
| `zjkBMFhNj_g` LLM talk (60 min) | 11,632 / 11,617 | 183 | 573 / 628 | 2,763 / 1,723 |

Most changed spans are spelling and number style: `30 minute` → `30-minute`,
`fine tuning` → `fine-tuning`, `one` → `1`, `et cetera` → `etc.`. These are
neither better nor worse. What matters for sentence cues is punctuation.
Without the hint, whisper sometimes writes whole windows in lower case with
no punctuation (`so at one loop here it's predicting this bracket only at 15
percent after two it's at 68 374 475 etc right up to sort of five and six
loops here seems to be the same so it's going to be…`). Then a sentence can
only be found from pauses. With the hint, 54 % (Dory) and 38 % (LLM talk)
fewer words sit in such runs.

Terms the hint corrected:

- `larger language models` → `large language models`, 6 times (23:40, 25:44, 25:49, 33:24, 35:01, 37:11). These are the title's words, and the speaker says "large".
- `competitionally` → `computationally` (4:27), `ChachiPT` → `ChatGPT` (46:54), `cloud` → `Claude` (47:54).
- `beef float 16` → `vFloat16` (Dory 2:37): still wrong (bfloat16), but closer.

Words the hint made worse:

- Dory 2:46 `pre-trained` → `free-trained`. The translation still says 사전 학습.
- Dory 12:36: `So the compute is roughly sort of the same for these` lost `the compute is roughly sort of`. Content lost.
- LLM 26:58: `administer a lot of different tests to these large language models` lost `tests to these large language models`. Content lost.
- LLM 56:27 `this like sleeper agent attack` → `this Lex Leber agent attack`.
- LLM 58:44 `jailbreak` → `shellbreak` and `backdoor` → `backdark`.
- LLM 34:18 `ChatGPT` → `ChashDBT`.
- LLM 0:32–0:50 `llama 2` → `LAMA2`. The glossary's mishearings map it back to Llama 2.

Title words written that were not said: none found. Every added `large`
(37 → 43) replaced a misheard `larger` in the phrase "large language models".
`NVIDIA's` (2 → 3) replaced `NVIDIA has` at 2:31; the audio is ambiguous
between the two. `Dory` went 2 → 1 (`Dory` ↔ `DORI` ↔ `Dori`; the speaker
says the same name each time).

After the re-makes, I checked all eight new transcripts the same way. In each,
I found the phrases of two consecutive title words that appear in the new
transcript with no such phrase within 6 s in the old one. There are 19 such
places. I read every one against the old transcript.

- 18 are speech: the old transcript has the same words under a different
  spelling or cut (`deep seek from scratch` → `DeepSeek from Scratch`,
  `two bit` → `2-bit`, `nemotron lives live stream` → `Nemotron Labs
  livestream`).
- One is not. At `WjhDDeZ7DvM` 24:47 the old cue was a spoken `Um,`. The new
  transcript writes `DeepSeek from Scratch` in its place, and the Korean renders
  it as `DeepSeek를 처음부터 구현하면`. This is a title phrase written that was
  not said. I counted it as a regression of the hint and list it below.

Judgement: better on balance, and kept. Sentence cues depend on punctuation,
and the hint restores much of it. Remaining: two short clauses lost and four
new misspelled terms in 75 minutes of audio, against six title-term
corrections and three other term fixes. These are listed again under
regressions below.

## Reading old and new Korean (done item 15)

I read the old and new Korean side by side, place by place, for each video.
Each place is one new sentence with the old cues that overlap it. The places
are:

- every place where the old version had a filler or an immediate repeat;
- every place where the old version had a term the glossary lists as misheard;
- at least 20 places per video in all, spread evenly, most of them where the
  old version cut a sentence.

The person narrowed this on 2026-11-19. Mid-sentence cuts are covered in full
by the counts above, and by sampling here.

| Video | Places read | Better | Same | Worse |
| --- | ---: | ---: | ---: | ---: |
| `QDX-1M5Nj7s` | 38 | 17 | 16 | 5 |
| `QWNxQIq0hMo` | 23 | 11 | 8 | 4 |
| `WJpeW9lCSUM` | 30 | 16 | 8 | 6 |
| `WjhDDeZ7DvM` | 75 | 43 | 23 | 9 |
| `_MEqhFwnp8o` | 263 | 200 | 48 | 15 |
| `vSCYL8ySh9g` | 28 | 13 | 10 | 5 |
| `wbc-oxFFV9c` | 60 | 26 | 29 | 5 |
| `zjkBMFhNj_g` | 57 | 25 | 28 | 4 |
| All 8 | 574 | 351 | 170 | 53 |

### QDX-1M5Nj7s — MIT Introduction to Deep Learning (2023) | 6.S191 (lecture)

Old 754 cues, new 1,316 cues in 513 sentences. Mishearings: FitS191 → 6.S191 and realu → ReLU. In the new transcript, whisper left long stretches without punctuation, spread across the whole hour. 86 sentences open in lower case. They are sentences the run-on limit ended at the cap where there was no comma, which is the designed fallback for an unpunctuated stretch.

I read 38 places: every old filler place (20), the two misheard-term places, and 16 spread evenly. Tally: better 17, same 16, worse 5.

| At | Old → new (Korean) | Verdict | Why |
|---|---|---|---|
| 0:09 | "…딥러닝 입문을 소개하게 되어 매우 기쁩니다." → "저희 모두 여러분께 'Introduction'을 소개하게 되어 매우 기쁩니다." | worse | The opening has no punctuation. The run-on limit ended a sentence between "Introduction" and "to Deep Learning", so the Korean names a course called "Introduction". |
| 0:23 | → "딥러닝 강의입니다. MIT 딥러닝 입문은…" / "먼저 제가, 저희가 하는 일에 대한 배경지식을…" | worse | The same cut leaves "딥러닝 강의입니다." on its own. "and what you're going to learn" goes to the next sentence. |
| 0:36 | → "올해 여러분이 배우게 될 내용에 대해" / "…이 분야의 기초를 배우게 됩니다." | worse | A clause separated from its verb by the forced end; "배우게" is said twice across the cut. |
| 0:48 | → "딥러닝과 인공지능 분야에 대해 배우게 될 것입니다." / "더 중요한 것은," / … | same | The first part repeats the previous cue's verb. The rest is now one sentence; the old version ended mid-clause ("강화하게 될 것입니다 / 실습 소프트웨어 랩의 일환으로 말이죠."). |
| 2:01 | "course on e-learning to the pod" "MIT의 이 포드(pod)에서 e-러닝 강의를…" → "the official introductory course on deep learning" "MIT에서 진행하는 딥러닝 공식 입문 과정입니다." | better | The new transcript heard the words. FitS191 → 6.S191 is applied in the Korean. |
| 11:34 | "you know the the technical part" in 3 cues → 5 parts of one sentence | better | Fillers and the repeat dropped; Korean order. |
| 15:05, 15:06, 15:10 | one 3-line cue, "매우, 매우" → 3 sentences, "매우 간단한" | better | Repeat dropped; one sentence per cue. |
| 18:34 | "the Relu function" → "the ReLU function," (Korean ReLU in both) | same | The old Korean already said ReLU. |
| 28:49 | "all the deep neural network is is" → "깊은 신경망이란 그저…" | same | — |
| 32:04, 32:09, 32:12 | one 4-line run-on with "right" ×2 → 3 sentences | better | Tics dropped. |
| 42:10 | 5 old fragments → 4 parts | better | — |
| 42:21 | "…also with W1. That's why we really call it back propagation" → "이 정보를 활용하기 때문에 이를 역전파라고 부르는 것입니다." | worse | The new transcript dropped "also with W1", and the Korean turns "that's why" into "because it uses this information": changed meaning. |
| 42:34, 42:46, 42:57 | one cue of 7+ lines → 3 sentences in parts | better | The old cue could not be read in its time. |
| 43:41 | "very, very deep" → "매우, ‖ 깊은" | same | The cut falls inside the repeat, leaving "매우," at the end of a part. |
| 51:17, 51:26 | old fragments "…분할할 수 있습니다 / 배치를…" → one sentence each; "very, very" → "아주" | better | — |
| 41:44 | "w2가 아니라, 죄송합니다, w1이 됩니다" → "경우에는 w2가 아니라 w1, ‖ 아니, w1이 됩니다." | worse | A forced end inside an unpunctuated stretch splits the speaker's self-correction ("not w1, but w, excuse me, not w2, but w1"), and the Korean garbles it. |
| 1:01, 9:12, 45:11 | old cue repeating "금요일" / fragments → one sentence in parts | better | — |
| 5:19, 13:26, 16:41, 19:41, 22:50, 26:36, 30:16, 32:20, 35:12, 38:52, 48:52, 53:12 | sampled; same sentences | same | Same meaning; the parts are in Korean order. |

### QWNxQIq0hMo — Build DeepSeek from Scratch: Series Introduction (talk, single speaker; re-made twice)

The first re-make was replaced by a second one on the run-on fix, as approved. This reading is of the second. Old 165 cues, new 313 cues in 142 sentences. Mishearings: Vijewara → Vijuara. The old transcript had unpunctuated stretches at 7:05, 9:40–9:55, 11:02 and 13:36. I read 23 places:

- every old filler or repeat place (7);
- the old cues that started or ended mid-sentence, spread evenly (16).

Tally: better 11, same 8, worse 4.

| At | Old → new (Korean) | Verdict | Why |
|---|---|---|---|
| 1:15 | "strongly strongly", "very very"; "이 DeepSeek 방식에서 말이죠" → "…매우 큰 ‖ 실수를 저지르고 있다는 점입니다, 이번 DeepSeek 열풍 속에서요." | better | The repeats are gone, and "DeepSeek wave" is heard right where the old had "way" ("방식"). |
| 3:08, 3:12 | "not very very deep" | same | The old Korean already dropped the repeat. |
| 9:46, 9:55 | "…다시 인터넷을 검색해 보니 구축 / DeepSeek를 처음부터 구축하는 것에 대해 다시 인터넷에서 검색해 보았고…" / "DeepSeek R1으로 돌아와서," → two clean sentences; "it it" dropped | better | The old repeated a clause across the cut and mistranslated "to DeepSeek R1". |
| 13:36, 13:42 | "음, 하지만 최종 목표는…" → "하지만 최종 목표는 여러분이 ‖ …" / "그러니 제가 MLA, 즉 Multi-Head Latent Attention을 보여드릴 때, ‖ …" | better | Filler dropped; one sentence per cue. |
| 0:04, 0:58, 4:23, 6:11, 8:37, 13:32 | — | same | Same meaning. |
| 2:16 | → "…그 영상들에서 저는 여러분께 ‖ 모든 구성 요소를 하나하나 가르쳐 드릴 것입니다. ‖ DeepSeek 아키텍처의 가르침이란," | worse | Changed meaning. The run-on limit ended the sentence at "and by teach,". Gemini moved "of the DeepSeek architecture" into the last part and made it "the teaching of the DeepSeek architecture is". |
| 3:41 | "이 재생 목록을 모두 마치고 나면 말이죠." → "이 과정을 모두 마친 후에 말이죠." / "전체 재생 목록입니다." | worse | The cap cut "this / entire playlist" inside the noun phrase. Gemini translated the last part as a sentence of its own ("it is the entire playlist"). |
| 5:17, 7:05, 9:40, 11:02, 11:53 | old cues cut mid-sentence ("그 이유는 바로 /", a duplicated "가르치기 위한 계획을 세웠습니다") → one sentence each | better | — |
| 7:44 | four old sentences → "이는 정말 모두를 놀라게 했는데 ‖ … ‖ 모델이었기 때문이며, 이는 2025년 1월에야 발표되었으니 ‖ 1년 정도의 연구가 있었거나 어쩌면" | worse | The new transcript has "R1. which" (a period before a lower-case word), so three sentences run together, and the run-on limit ends the last part on "어쩌면" ("or maybe"). |
| 9:07 | "…짚어주는 영상 말이죠, 그렇죠?" → "…짚어주는 영상 말입니다." | better | The tic "right?" is dropped. |
| 12:37 | → "…비로소 우리는 어떻게 모든 ‖ DeepSeek의 요소가 처음부터 조립되는지 알게 될 것이고 ‖ 이 과정의 끝 무렵에는" | worse | A run-on stretch. "모든" ("every") is cut away from its noun across parts, and the sentence ends on "이 과정의 끝 무렵에는" ("towards the end of this"). |

### WJpeW9lCSUM — NVIDIA's Nemotron Bolt 74B-A9B 2 Bit (talk, single speaker)

Old 309 cues, new 499 cues in 240 sentences. Mishearings: 4950 → 4090 and GDUF → GGUF. Its first 34 writing windows were made before the run-on fix, and the video could not be re-run because its remaining room (125 KRW) is below the 139 KRW its run cost. I read 25 places: all 2 filler places, all 3 term places, and 20 spread evenly. I also looked at every place my earlier scan flagged: duplicates, words cut off a sentence end, and lost clauses.

| At | Old → new (Korean) | Verdict | Why |
|---|---|---|---|
| 0:00 | "경험칙은 꽤" / "간단했습니다. 4-bit로…" → "자, 오랫동안 …경험칙은" / "꽤 간단했습니다." | better | The old cue ended mid-word-group, and the next one carried two sentences. |
| 1:16 | "It's a mixture of experts models." at 76.7 s and 78.4 s | worse | The sentence is shown twice. Two overlapping windows both kept it; this is a stitching duplicate from the pre-fix part of the run. |
| 1:43 | "your other apps." → "다른 앱들과 함께 실행할 수 있습니다." after "…24GB 장치 하나에서 함께 실행할 수 있다는 것입니다." | worse | The sentence's last two words were cut off as their own cue, so the Korean repeats "함께 실행할 수 있습니다". Pre-fix run-on cut. |
| 2:09 | "these / days." → "…지도를 얻은 셈입니다." / "날들입니다." | worse | A one-word stray cue whose Korean ("days are") means nothing. Pre-fix. |
| 2:17 | orphan "Hugging Face에서 보기 시작한 2-bit 범위입니다." → 4 parts in Korean order | better | The Korean moved "Hugging Face" one part earlier, which stays inside the sentence. |
| 2:58 | "as they come / out." → "…확인하실 수 있을 겁니다." / "나옵니다." | worse | A stray one-word cue. Pre-fix. |
| 3:01 | GDUF; one 4-line cue with "(PTQ)" → 3 parts | better | Term correct in the Korean (as before). The added parenthetical is gone, and each cue fits two lines. |
| 3:11 | orphan "더 적은 비트로요." → "…더 적은 비트로 반올림한 것입니다." | better | The fragment is gone. |
| 3:29, 5:48, 10:24, 20:24 | same sentence, same Korean | same | — |
| 4:07 | "where you see a huge / amount of deterioration." → "상전이 구간에서 큰 변화가 나타납니다." / "성능 저하가 크게 나타납니다." | worse | Pre-fix cut. The Korean states the change twice instead of once across two parts. |
| 4:32 | 2 cues → 3 parts | better | Korean order; nothing cut. |
| 6:38 | "…바로 품질입니다." / orphan "설계상의 선택입니다." → 3 parts ending "…설계상의 선택이라는 점입니다." | better | The old Korean misread the sentence ("the third reason is quality"). |
| 7:32 | "실제로" / "각 레이어에 가장 적합한 양자화 수준으로" / "레이어를 구성하는 것이 타당합니다." | same | The first part is one word; the meaning is kept. |
| 8:25 | "몇 개의 Mixture of Experts (MoE)를 유지할지" → "얼마나 많은 전문가를 유지할지" | better | The old version replaced "experts" with the glossary term. "층별로 학습합니다," is slightly stiff. |
| 9:29 | "(지도 미세 조정)", "(강화 학습)" added → removed | better | No added content. |
| 10:06 | "…for the updates so the model itself… instead of just getting rounded at the end" → "…사본은 유지해야 합니다..." / "…양자화 그리드에 맞춰…" | worse | Lost content: "for the updates" and "instead of just getting rounded at the end" are not in the new English, because the transcript differs at this point. |
| 11:31 | 1 cue → 4 parts, ending "…좋아야 한다는 것을요." | same | Postposed object, natural in speech. |
| 12:35, 14:52, 16:54, 19:06 | long cues → sentence parts | better | No cue carries two sentences or ends mid-clause. |
| 13:39 | "right?" kept as "…높겠죠?" → "…높습니다." | better | Tic dropped. |
| 16:00 | "where okay that's" → "…볼 수 있는데, 자, 이건," | same | The false start is kept as "자, 이건,", and the old version kept it too. |
| 17:57 | orphans "모두 얻게 될 것입니다" / "일종의 에이전트 도구 사례들이 작동하고 있습니다." → 4 parts, one sentence | better | The old Korean contradicted itself. |
| 18:32 | "I find that it sometimes can be" → "가끔씩" / … / "예를 들면 여기 있는 프롬프트 주입 저항성 같은 경우입니다." | better | — |
| 18:42 | "all all eight" → "8점 만점에 8점" | better | Repeat dropped. |
| 20:57 | "495090" / "4950, 90" → "3090이나 4090" | same | The old Korean already said 4090; the mishearing is right. |

### WjhDDeZ7DvM — Build DeepSeek from Scratch: Series Introduction (lecture)

Old 408 cues, new 770 cues in 344 sentences. I read every old filler/repeat place (9 F), every misheard-term place (52 T/MT), and 14 sampled mid-sentence places: 75 in all. Tally: better 43, same 23, worse 9.

| At | Old → new (Korean) | Verdict | Why |
|---|---|---|---|
| 1:34–1:46 | "명심해야 할 점은 그 핵심에는," + "LLM이 무엇인지에 대해 기억해야 할 핵심은, …엔진이라는 점입니다" → "명심할 점은," / "LLM의 핵심이 무엇인가 하는 것입니다." / "단어 시퀀스를 입력받아" / "…예측하는 엔진입니다." | same | Old repeated "핵심" twice; the new version cuts the sentence at the speaker's pause after "is,", so the second sentence opens on "is that…". The meaning is kept. |
| 10:26–10:45 | one 4-line cue (ChatGPD, RLHF, annotators) → 5 short cues | better | Term fixed in the English too; each cue is now one sentence, or half of one, on two lines. |
| 11:27 | DeepSeq → DeepSeek in three parts | better | Same meaning, now in order. |
| 11:39 | "그럼" / "DeepSeek이 구축한 다양한 LLM을 살펴보겠습니다." | same | The first part is only "그럼", which is short but correct. |
| 11:43 | deepseq.com → deepseek.com | better | Both the English and the Korean are now spelled right. |
| 11:54, 16:23, 17:19, 20:30, 29:48 | DeepSeq/GPT-40 fixed in the English; Korean already correct | same | The Korean was already correct. |
| 30:53, 30:56, 31:07 | one cue holding three sentences (PTX article) → one sentence per cue, DeepSeq fixed | better | The old cue ran past four lines. |
| 12:55 | 3 cues → 4 parts; "OpenAI의 최고 모델과 대등한 성능을 달성했습니다." | better | No cue ends mid-clause. |
| 13:19 | orphans "그것은 추론 모델이었습니다" / "하지만…" → "…DeepSeek R1은" / "추론 모델이었지만" / "…보였습니다." | better | One sentence in Korean order. |
| 13:54 | "…아주 멋진 일이죠, 그렇죠?" → "정말 대단하지 않나요?" | better | The tic "right?" was dropped without losing meaning. |
| 13:55 | "as we saw the size scaling law." → "우리가 보았던 크기 조정 법칙과 같습니다." | worse | The new transcript ends a sentence after "law"; the Korean then says "it is the same as the scaling law", which is a changed meaning. |
| 14:00–14:09 | long cue → "…예외는 아니었습니다." / "…6,710억 개의 파라미터를 가지고 있습니다." / 3 parts for R1 | better | Each part reads in order. |
| 14:18–14:42 | version 2/3, R1, progression (DeepSeq) | better | Fixed in the Korean; "broke the internet" attaches to R1 within one sentence. |
| 17:11 | "Alright, so now let us do a bit" → "자, 이제" / "…비교해 보겠습니다." | same | A short first part; the meaning is kept ("AII models" → "LLM 모델" as before). |
| 17:23–17:30 | 2 long cues → 5 parts | better | Shorter cues, correct order. |
| 18:19 | "as much uh better performance" → "훨씬 더 나은 성능을 보이면서" / "가격도 합리적입니다." | better | The filler is gone and the meaning is clearer. |
| 18:32–18:45 | "…비용은 훨씬 저렴합니다." / "DeepSeek-V3와 GPT-4o를 비교해 보면," | worse | The cut after "if" puts the condition after its main clause, so the cue ends on "비교해 보면,". All the content is there, but in inverted order. |
| 18:48 | 2 cues → 5 parts | better | "수직으로" is slightly literal; the content is complete. |
| 19:14 | "…긴장감이 존재하지만," / "이 모델은 마침내 그 격차를 해소한 사례입니다." | better | One sentence; "you know" was dropped. |
| 19:22 | "GPT-4Go" → Korean "GPT-4" (old: GPT-4o) | worse | Wrong term correction. The glossary listed GPT-4Go → GPT-4, but the slide compares with GPT-4o. The old fragment "Which / 어떤" is gone (better). |
| 19:33 | DeepSig → DeepSeek in the Korean (the old Korean kept "DeepSig") | better | Term fixed. |
| 20:17–20:25 | Lama → Llama; "Meta에서 공개한 소스 모델입니다." orphan → one sentence | better | Term fixed, fragment gone. |
| 20:34 | "…보유하고 있으며" / "실제로 Llama의 700억 파라미터 모델이" / "…뛰어난 강력한 결과를 보여줍니다." | same | Content moved between the parts of one sentence (allowed). |
| 20:43, 20:55 | LAMA → Llama; orphan "파라미터입니다." gone | better | Terms fixed, fragments gone. |
| 21:08 | "multi-tocal / 예측." → "Multi-Token Prediction" | better | Mishearing corrected; the orphan "예측." is gone. |
| 21:17 | Lama → Llama | better | Term fixed. |
| 21:20 | "…Lama와 비교했을 때의 내용입니다." → "자, 그럼 DeepSeek와 GPT-4를 비교해 보면" / "Llama와 비교했을 때도 마찬가지입니다." | worse | It reads as starting a comparison, but the speaker is closing one. |
| 22:34 | "…저울질해야 합니다." ("right" kept as 잘) → "…고려해야 합니다." | better | The tic was dropped. |
| 22:37 | GPT-44 → Korean "GPT-4" (old: GPT-4o) | same | The audio could be either GPT-4o or GPT-4; uncertain, not counted as a regression. |
| 22:44 | one 4-sentence cue → 3 parts | better | — |
| 23:18 | GPT-40 → GPT-4o in 2 parts | same | Already correct. |
| 24:47 | "Um," / "음," → "DeepSeek from Scratch" / "DeepSeek를 처음부터 구현하면" | worse | The title hint wrote title words that were not said, and the Korean adds content. |
| 26:19 | "which which … 어떤 부분이 결정하는" → 3 parts | better | Repeat dropped; the old Korean was garbled. |
| 28:28 | 2 cues → 2 sentences | better | — |
| 28:38 | "rotary … which which is the fifth key innovation which / The DeepSeek architecture implemented." | same | The source itself breaks the clause; the Korean "…관한 것으로," / "DeepSeek 아키텍처에 구현되었습니다." keeps the meaning. |
| 29:41 | 3 parts | better | — |
| 32:26–32:38 | "…Llama 등과 비교했을 때 말이죠." / "the cost requirements which they use to train the models." → "그들이 모델을 학습시키는 데 사용하는 비용 요건입니다." | worse | The run-on limit ended the sentence at its last comma, so the closing phrase stands alone as a fragment. |
| 32:41 | "So it's like..." → "그러니까," | same | — |
| 35:01 | "you you go through" → "이전 시리즈를 먼저 학습했다고 가정합니다." | better | Repeat dropped. |
| 35:07, 35:13 | "phase three gpu / optimization text" → "3단계는 GPU 최적화입니다." | better | The misheard "text" was dropped from the Korean, and there are no orphans. |
| 38:16 | GRPO in 2 parts | same | — |
| 38:22 | PTA → PTX | better | Mishearing corrected; the expansion "Parallel Thread Execution" is not repeated in the Korean (minor). |
| 0:04 | "여러분 안녕하세요," / "'Build DeepSeek from Scratch' 시리즈의…" | same | Said in the audio; the series name is left in English. |
| 2:44 | "your effort will take you 2." "당신을 2로 이끌 것입니다" → "여러분의 수고는 결실을 맺을 것입니다." | worse | Lost content: the example is about predicting the next token ("to"), and the new Korean drops it. |
| 5:14, 16:53, 24:55, 30:23, 32:56 | long cues → 2–3 parts | better | Korean order; no fragments. |
| 7:48, 10:00, 14:51, 22:17, 27:16, 37:06 | sentence split, same content | same | — |

### _MEqhFwnp8o — Ask the Experts: Dory Looped Transformer Model (live stream, four speakers)

Old 678 cues, new 1,008 cues in 564 sentences. Mishearings: "loopy boys" → Looped Transformer (wrong: that is the host's joke name, though the Korean did not use it), "Star Elastics" → Star Elastic, "4B model" → 4B. I read 263 places: every old filler or repeat place (239), every misheard-term place (12, marks F/T), and 16 spread evenly. Tally: better 200, same 48, worse 15.

| At [mark] | Verdict | Why |
|---|---|---|
| 1:05 FM, 1:08 FM, 1:13 FM, 1:17 FM, 1:21 FM, 1:22 FM, 1:25 FM, 1:34 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 1:38 FMT | worse | The subject ("these loopy boys", i.e. looped models) is dropped, leaving "이 도대체 무엇인지" without it. |
| 1:47 FMT, 1:50 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 1:53 F | same | An unfinished "We have..." stays "저희는…". |
| 1:53 F, 1:54 F, 1:55 F | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 1:56 F | same | A joke line, kept as is. |
| 1:57 F, 2:05 F, 2:06 F, 2:31 FM, 2:38 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 2:39 FM | same | "Okay, we have..." → "자,". |
| 2:40 FM, 2:42 FM, 2:45 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 2:48 FM | worse | "Working along Context" (long context) became "Chain of Thought와 함께 작업하며": a wrong term. |
| 2:52 FM, 3:22 FM, 3:28 FM, 3:35 FM, 3:38 FM, 4:39 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 4:56 FM | same | The sentence ends where the speaker paused ("…눈치채지 않을 수 없네요."); the meaning is kept. |
| 5:09 FM | same | The continuation; the meaning is kept. |
| 6:55 FM, 7:04 FM, 7:17 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 8:41 FM | worse | The new transcript drops "works from a user standpoint", so the question loses its end (lost content). |
| 8:52 FM, 8:55 FM, 8:58 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 10:22 T | same | No term at this point; same. |
| 10:25 T | better | "star elastics" → "Star Elastic" (term fixed). |
| 10:34 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 10:49 FM | same | Same meaning; "the loop process" is rendered as Looped Transformer. |
| 10:58 FM, 13:45 FM, 13:59 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 14:40 FM | same | Same meaning; "because" carries over to the next sentence. |
| 15:23 FM, 15:47 FM, 15:50 FM, 16:19 FM, 16:34 FM, 16:46 FM, 16:58 FM, 17:02 FM, 17:03 FM, 17:08 FM, 17:11 FM, 17:20 FM, 17:26 FM, 17:31 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 17:34 FM | worse | "Nemo Switchyard" became "Nemotron Switchyard": a term changed without evidence. |
| 18:19 F, 18:21 F | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 19:55 F | worse | The name is heard as "Psy" here but as "Cy" at 21:34; the Korean follows the mishearing. |
| 19:59 F, 20:03 FM, 20:09 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 20:20 FM | worse | The new transcript drops the end of the question ("…my scores on whatever internal benchmark got better with loops"): lost content. |
| 20:26 FM, 20:35 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 20:43 FM | same | The source is garbled; both versions follow it. |
| 20:52 FM | worse | "…otherwise it doesn't know how to deal with it, right" was cut into a sentence of its own and translated as a question ("이를 어떻게 처리할까요?"): changed meaning. |
| 20:53 FM | same | "So that's the fact." → "그게 사실입니다." |
| 20:56 FM, 21:00 FM, 21:03 FM, 21:08 FM, 21:14 FM, 21:21 FM, 21:34 FM, 21:44 FM, 21:50 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 21:57 FM | same | "A omniscience" (a benchmark) is wrong in both ("전지전능함" / "전지적 모델"). |
| 22:05 FM, 22:29 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 23:45 FM | worse | The name is written as misheard ("Abishake"); it is Abhishek elsewhere. |
| 23:54 FM, 23:56 FM, 23:58 FM, 24:11 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 24:23 FM | worse | The run-on limit cut the question at "or even better,", so the option "we know there's not going to be a benefit" reads as a statement ("…이득이 없을 것임을 우리는 알고 있습니다"): changed meaning. |
| 24:28 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 24:31 FM | same | "Shantanu, Yeah." → "샨타누, 네." |
| 24:32 FM, 24:44 FM, 24:50 FM, 24:52 FM, 24:56 FM, 25:00 FM, 25:02 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 25:27 F | worse | The part "and the more in the later stage of training," became "학습 후반 단계일수록 그렇습니다", which contradicts the next part: changed meaning. |
| 25:42 F, 26:47 FM, 26:48 FM, 26:50 FM, 26:54 FM, 27:01 FM, 27:04 FM, 27:49 FM, 27:52 FM, 27:56 FM, 27:58 FM, 28:06 FM, 28:09 FM, 28:15 FM, 28:28 FM, 28:30 FM, 28:40 FM, 29:06 FM, 29:16 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 29:23 FM | same | The chat name differs by hearing ("Stalker" / "Thakar"); unknown which is right. |
| 29:28 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 29:29 FM | worse | "Everyone has said [the same]. Maybe Amanda, we can get him full screen" became "everyone suggested asking Amanda": changed meaning. |
| 29:34 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 29:36 FM | same | "view" (the guest's scenery) is "관점" in both. |
| 29:40 FM, 29:46 FM, 29:54 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 30:00 F | worse | "MCTs" (tree search) became "COT": a wrong term. |
| 30:16 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 30:18 FM | worse | "Maybe Christian, up for this one" was heard as "Maybe question up for this one", and the Korean ("…아마도 그럴 것 같네요") means nothing. |
| 30:19 FM | same | "view" is "견해" (both versions miss the scenery sense). |
| 30:22 F, 30:28 FM, 30:32 FM, 30:43 FM, 30:46 FM, 30:51 FM, 30:56 FM, 30:59 FM, 31:08 FM, 31:12 FM, 31:16 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 31:23 FM | same | "One of the most..." → "가장 중요한 것 중 하나는..."; the translator completed the phrase. |
| 31:27 FM, 31:31 FM, 31:37 FM, 32:33 MT, 32:45 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 32:50 FM | same | "last loop" became "마지막 Looped Transformer" in both versions. |
| 32:54 FM, 32:55 FM, 32:57 FM, 33:03 FM, 33:06 FM, 33:11 FM, 33:15 FM, 33:19 FMT | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 33:23 MT | better | "get clawed" → "Get Claude": the old version dropped it. |
| 33:24 MT | same | Same meaning. |
| 33:25 FMT, 33:29 FM, 33:30 FM, 33:34 FM, 34:08 FM, 34:12 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 34:25 FM | same | The source is garbled; both Korean versions are hard to read. |
| 34:45 MT | same | Same meaning. |
| 35:01 MT | same | Same meaning. |
| 35:31 FM | same | The name differs by hearing ("Rohan Nuri" / "Rahane Nouri"). |
| 35:35 FM, 35:38 FM, 35:43 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 35:47 FM | same | The false start "is it better or" is folded in as "아니면 더 나은 방법이 있는지"; this is close. |
| 36:00 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 36:10 F | better | "Sean to new" → "Shantanu" (name fixed). |
| 36:16 FM, 36:21 FM, 36:36 FM, 36:46 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 36:47 FM | same | "Answer again, guys" is unclear in both versions. |
| 36:49 FM, 36:50 FM, 36:56 FM, 36:59 FM, 39:33 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 39:42 FM | same | "Yeah." → "네." (an interjection kept as its own cue). |
| 39:43 FM | same | "Yes." → "네." |
| 39:44 FM | same | "Okay." → "자," |
| 39:45 FM, 39:49 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 39:56 FM | worse | "Questionable Commands" is a chat username (see 2:42), but it was translated as "의문스러운 명령": changed meaning. |
| 40:12 F, 40:56 FM, 41:02 FM, 41:08 FM, 41:10 FM, 41:18 FM, 41:24 FM, 41:30 FM, 42:08 FM, 42:14 FM, 42:17 FM, 42:24 FM, 43:00 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 43:03 FM | worse | "an MoE Dory equivalent" became "MOE나 Dory에 해당하는 것" (MoE *or* Dory); the old Korean had it right. |
| 43:13 FM, 43:16 FM, 43:21 FM, 43:51 FM, 43:56 FM, 44:06 FM, 44:11 FM, 44:19 FM, 44:31 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 44:42 FM | same | The tail of a run-on sentence ("most accessible option to building a claw") stands alone; it reads as a list item. |
| 44:47 FM, 45:47 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 45:50 FM | same | "Okay." → "자," |
| 45:51 FM | better | "on video" → "at NVIDIA" (term fixed). |
| 45:53 FM, 46:01 FM, 46:07 FM, 46:09 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 46:14 FM | same | "Okay." → "자," |
| 46:14 FM, 46:15 FM, 47:19 FM, 47:25 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 47:32 FM | better | "shot to do the same" → "Shantanu, same question" (name fixed). |
| 47:56 MT | same | Same meaning (Dorie → Dory). |
| 48:54 FM, 48:55 FM, 48:56 FM, 48:57 FM, 49:00 FM, 49:03 FM, 49:07 FM, 49:10 FM, 49:14 FM, 49:18 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 49:23 FM | same | The run-on limit cut "next / week" across two sentences, so the Korean says "won't see you" in both. |
| 49:34 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 49:44 FM | same | Same farewell. |
| 1:01 M | same | Same greeting. |
| 4:28 M | same | Same. |
| 6:41 M | same | A stray "directions." cue, as in the old version ("방향들을요"). |
| 9:50 M | same | Same. |
| 11:52 M | same | Same. |
| 13:53 M | same | Same. |
| 16:05 M | same | Same. |
| 19:20 M | same | Same. |
| 23:25 M | same | Same. |
| 26:19 M | same | Same. |
| 31:45 M | better | The false start was dropped; one sentence. |
| 35:21  | same | Same. |
| 38:44 M | same | Same. |
| 40:46  | same | The transcript differs here ("Is it available? In build.nvidia.com"); I can't verify which is right. |
| 44:02 M | same | Same. |
| 46:25  | same | Same. |

### vSCYL8ySh9g — NVIDIA's looped 4B model (talk, single speaker; re-made twice)

The first re-make was replaced by a second one on the run-on fix, as approved. This reading is of the second. Old 202 cues, new 314 cues in 132 sentences. Mishearings: DOI → Dory, vFloat16 → bfloat16. I read 28 places:

- every old filler place (5);
- every place marked as a misheard term (7). One is real (DOI at 6:14); in the other six my term check matched "DOI" inside "doing";
- the old cues that started or ended mid-sentence, spread evenly (16).

Tally: better 13, same 10, worse 5.

| At | Old → new (Korean) | Verdict | Why |
|---|---|---|---|
| 4:20 | "…서로 다른 가중치 세트가 아니라 / 각 블록마다 매번, 기본적으로 동일한 파라미터 세트가 그저 / 여기서 강화됩니다." → "…그 수정 사항이 ‖ 매번 각 블록마다 서로 다른 가중치 집합을 사용하는 대신, ‖ 기본적으로 동일한 가중치 집합이 ‖ 여기서 강화된다는 것입니다." | better | One sentence; the verb is in the last part. |
| 6:14 | DOI → Dory | same | The old Korean already said Dory. |
| 9:44 | "네, 그래서 이 부분이 저에게는, / 가장 궁금했던 부분입니다." → "그래서 이 부분이 ‖ 제가 가장 궁금했던 점인데, ‖ …" | same | — |
| 12:04, 12:07 | → "이 방식은 전혀 효과가 없으며…" / "아마도 그 이유는 ‖ 모델의 가중치가 충분하지 않기 때문일 것입니다." / "26B 버전과 비교하면 말이죠." | same | "Compared to the 26B version" is now its own short sentence; nothing lost. |
| 12:37 | "따라서 이들의 연산량은 대략 비슷합니다." → "이 모델들도 마찬가지인데," | worse | Lost content: the new transcript has "So the same for these", without "the compute is roughly sort of". The first re-make, without the run-on fix, lost it too. It came from the title hint: the hint-off transcript of the same audio keeps it. |
| 13:17, 13:25 | — | same | — |
| 14:03 | "…저는 이런 종류의 연구용 버전이 / 사람들이 프로덕션에서…" → one sentence in 3 parts | better | The old cut the sentence after its subject. |
| 14:09 | → "하지만 이런 종류의 모델을 미세 조정하는 방법을 살펴볼 때 ‖ 다양한 루프 횟수 등에 맞춰 조정하는 것은 중요하다고 생각합니다." | worse | Content added. The run-on limit ended the sentence at "etc.," before "you could imagine…". Gemini completed the cut clause with "중요하다고" ("it is important"), which the speaker did not say. |
| 14:29 | → "어쨌든 모델 카드에 대한 링크를 ‖ 설명란에 모두 적어 두었습니다." | same | "And all the details" is folded into "모두" ("all"). |
| 14:33 | "루프 트랜스포머" → "잠재 순환 트랜스포머" | worse | Wrong term: "loop transformers" became "latent recurrent transformers" (glossary term). |
| 0:00, 0:56, 2:24, 3:17, 4:11, 5:13 | old cues cut mid-sentence ("…서로 다른 양의 루핑이 일어나고 있는 것입니다. / 모델입니다.", "open mdw 라이선스로 공개했습니다. / 라이선스이므로…") → one sentence or one part per cue | better | The old repeated or orphaned words at each cut. At 2:24 OpenMDW is now one name. |
| 1:44, 6:02, 12:59 | — | same | — |
| 6:52 | "…마지막 루프는 변하지 않습니다 / 전혀 변하지 않습니다." → one sentence | better | The old said "does not change" twice. |
| 7:47, 9:25, 10:07, 10:55 | old fragments, a duplicated clause at 9:25 → one sentence each | better | — |
| 8:35 | → "하지만 항상 그런 것은 아니며, ‖ 이것이 ‖ 단순히 ‖ 추론 능력을 높이는 것과 ‖ 같다고 말할 수는 없습니다." | worse | A 140-character sentence was cut at five pauses. Two parts show one word for 0.8 s and 2 s ("이것이", "단순히"). The meaning is kept, but it is hard to read. |
| 11:40 | "따라서 지시 이행과 긴 컨텍스트… / 이 모델이 성능을 잃는 경향이 있는 부분은…" → "지시 이행과 긴 문맥, ‖ SWE Bench 검증 같은 작업에서는, ‖ 이 모델이 뒤처지는 부분은 ‖ …지식이 필요한 작업들입니다." | worse | Changed meaning. The new transcript wrote a comma where the speaker ended a sentence, so two sentences became one. The Korean now says the model falls behind within instruction following, where the speaker said it wins there. |

### wbc-oxFFV9c — Give Me 1 Hour, I'll Make Deep Learning Click Forever (lecture, single speaker)

Old 885 cues, new 1,490 cues in 954 sentences. The glossary's mishearings: Atom → Adam, Bahtenau → Bahdanau, beef float 16 → bfloat16, Burt → BERT, Marion → Marian, Mixtrel → Mixtral, realu → ReLU, Seek to Seek → Sequence to Sequence, "Suitskiver, Vignols, and Lay" → "Sutskever, Vinyals, and Le", Word2Vex → Word2Vec. The old transcript of this video had long unpunctuated stretches (from 19:58, 40:07, 43:59, 60:08 and 65:38 on). The new one has fewer of them, so most of the gain here is one sentence per cue where the old cues ran sentences together.

The first full run failed on one cue (1388, 70:46, "First, what does GPT keep as it writes and why does it grow?"). Each time it was asked, Gemini returned two segments for this one-part sentence: id 1388 and an invented 1389, splitting the sentence at "and why". The id check refused those replies. It was continued twice; the second continue got a one-segment reply. The Korean is "먼저, GPT가 글을 쓸 때 무엇을 유지하며, 왜 그 크기가 커지는지 알아보겠습니다."

I read 60 places:

- every old filler or repeat place (10);
- every misheard-term place (34);
- 16 spread evenly.

Tally: better 26, same 29, worse 5.

| At | Old → new (Korean) | Verdict | Why |
|---|---|---|---|
| 4:02, 4:04 | "1.1 곱하기 3.1 더하기 3.1 빼기 4.9를 계산하면 1.3이 됩니다" (one cue) → "0.1을 취하세요." / "1." | worse | The new transcript put sentence ends inside the spoken arithmetic ("take the dot. 1. 1."), so two cues are fragments, and "0.1" is wrong. The old cue read as one calculation. |
| 4:04 (58) | → "3.1 더하기 3.1, 빼기 4.9는 1.3으로 0보다 크니, 맞습니다." | same | The rest of the calculation reads correctly. |
| 4:44, 4:48, 4:52 | a cue holding two sentences, ending on "ReLU는," → one sentence each; "Real U" → ReLU in parts "ReLU는 ‖ 양수는 그대로 유지하고 음수는 0으로 바꿉니다." | better | One sentence per cue; the verb is in the last part. |
| 5:14, 5:16, 5:20 | flagged for "0, 0, 0" | same | Not a filler; the same sentences. |
| 10:56, 11:06, 11:11 | Atom → Adam, "TAN-H" → tanh | same | The old Korean already had Adam and TanH. The new parts are in Korean order. |
| 18:49, 18:53 | realu → ReLU | same | The old Korean already had ReLU. |
| 19:58, 20:03 | unpunctuated run-on cues ("…필터당 하나씩 생성된 후 풀링을 통해 크기가 줄어듭니다. / 그것들을 처리하고…") → "합성곱과 ReLU를 거치면 ‖ 필터당 하나씩, 특징 맵의 스택으로 변환됩니다." / "그다음 풀링 계층이 이를 축소하고 이 과정이 반복됩니다." | better | The old cut "them" off from "shrinks" and translated it as "그것들을 처리하고" ("process them"). |
| 40:04, 40:06 | "여기 첫 번째 답변이 있습니다." / "9번입니다." | same | — |
| 40:07 | "시퀀스 투 시퀀스(Sequence to sequence) 또는 Seq2Seq" → "Sequence to Sequence ‖ 또는 2014년에 나온 Sequence to Sequence 모델입니다." | worse | Wrong term correction: the glossary maps "Seek to Seek" to "Sequence to Sequence", so the speaker's two names (sequence to sequence, seq2seq) became the same name twice. The old version had it right. The sentence that follows ends in an unpunctuated stretch on "그러면 이 벡터는", carried on in the next cue. |
| 42:47, 42:53 | Bahtenau → Bahdanau | same | The old Korean already had Bahdanau. |
| 43:59, 44:01 | "그것의 한 단계를 아래에서 천천히 그려보겠습니다. 아래쪽에는…" (run-on) → "그중 한 단계를 천천히 그려 보겠습니다." / "하단에는 Sequence to Sequence의 RNN인 인코더가 있고, ‖ …" | better | The old joined two sentences and attached "at the bottom" to the wrong one. Here "seek to seek" alone is expanded correctly. |
| 44:09 | "…end 토큰까지" → "…N 토큰 뒤의 H8까지 이어집니다." | worse | The new transcript heard "end token" as "N token"; the Korean says "N 토큰". The old had "end 토큰". The segmentation is better, but a term is lost. |
| 44:18 | "plain Seq2Seq는 오직 / 마지막 하나지만 이제 상단에 8개를 모두 유지하는 것은 디코더이며…" → "기존의 Sequence to Sequence는 마지막 하나만 유지했지만, 이제는 여덟 개를 모두 유지합니다." | better | The old cut merged two sentences into a wrong one. |
| 46:19, 46:46, 56:22 | "Marion" → "Marian" | better | Term fixed by the mishearings list. |
| 56:20 | "그 첫 번째 작업은 번역이었습니다." | same | — |
| 58:44 | "BERT의 Both Ways 게임은…" → "BERT의 양방향 방식은 ‖ 11가지 언어 작업에서 새로운 기록을 세웠습니다." | better | The old kept the mishearing "Both Ways game" as a name; the new reads it as "both ways", meaning bidirectional. |
| 60:08, 60:22, 60:25, 60:28, 60:37 | three-line run-on cues ("…cls는 실제 단어가 아니지만, BERT는 모든 문장 앞에 이 토큰을 넣습니다. / 입력값으로 들어가고…") → one sentence or one part per cue | better | The old cut "every / input" across cues. |
| 60:39, 60:40 | "여기 실제 BERT가 있습니다. 그런 방식으로 미세 조정을 하면 / 영화 리뷰에서…" → "여기 진짜 BERT가 있습니다." / "영화 리뷰를 읽으면서 그런 방식으로 미세 조정을 하세요, ‖ 우리 고양이에 관한 두 문장을요." | same | Both misread "fine-tuned that way on movie reviews": the old as a condition ("if you fine-tune"), the new as a command ("do the fine-tuning"). |
| 62:34 | "Burt" → BERT | same | The old Korean already had BERT; the new sentence is in 4 parts. |
| 67:52, 67:59, 68:01 | "the the router" in a cue cut mid-clause ("각 단어는 오직 / 두 명의 전문가에 대해 첫 번째 단어가 들어오면…") → three sentences; "첫 번째 단어인 'the'를 따라가 보세요." | better | The repeat is resolved: the second "the" is the word being followed. |
| 69:54, 70:06, 70:08 | Mixtrel → Mixtral | same | The old Korean already had Mixtral. |
| 0:00 | title sentence | same | — |
| 4:11 | "0보다 크므로 네, 이것이 퍼셉트론 아키텍처의 전부이며 모든 다이어그램은 이 키를 따릅니다." → "이것이 퍼셉트론의 전체 구조입니다." | better | The old cue held three sentences and mistranslated "follows the key along". |
| 22:51, 42:12, 60:53 | old cues starting or ending mid-sentence → one sentence each | better | — |
| 27:53 | "king 마이너스 / man 더하기 woman" across two cues → "숫자로 표현하면 킹 마이너스 맨 플러스 우먼입니다." | better | The expression is no longer cut in two. |
| 32:15 | "메모리가 u와 TanH를 거쳐 되돌아가기 때문에" → "메모리가 다시 여러분을 거쳐 tanh를 통과하므로" | worse | The new transcript heard the weight "U" as "you", and the Korean says "through you": changed meaning. The sentence is also in an unpunctuated stretch and its last part ends on "단어가 많아지면" ("as the words pile up"). |
| 65:38 | "…프랑스어로 번역하는 것입니다. / 텍스트와 2020년의 GPT-3는…" → "프랑스어로 번역한 다음, 영어 텍스트와 프랑스어 텍스트를 나열하세요." | better | The old cut "the French / text" across cues and joined it to the next sentence. |
| 8:25, 13:10, 17:58, 36:57, 46:48, 51:03, 55:23, 70:19 | sampled; same sentences | same | Same meaning. |

### zjkBMFhNj_g — [1hr Talk] Intro to Large Language Models (talk, single speaker)

Old 892 cues, new 1,409 cues in 669 sentences. The glossary's mishearings include Chat-APT/ChashDBT/Chash GPT → ChatGPT, CLAWD/class series → Claude, BART/BARD → Bard, LAMA270b → Llama 2 70B, and "Lex Leber agent attack" → "backdoor agent attack" (wrong; the speaker says "sleeper agent attack"). I read 57 places:

- every old filler place (18);
- every misheard-term place (19);
- 16 spread evenly;
- the 4 places the title-hint comparison flagged (26:58, 34:18, 56:27, 58:40).

Tally: better 25, same 28, worse 4.

| At | Old → new (Korean) | Verdict | Why |
|---|---|---|---|
| 0:25, 0:29 | one cue holding two sentences, "…그렇죠?" → one sentence each; "right?" dropped | better | Tic dropped; one sentence per cue. |
| 0:32, 1:24, 1:31, 4:37 | llama 270b / Lama 270b / LAMA270B → "Llama 2 70B" in both | same | The old Korean was already right. |
| 2:23, 2:26 | "그리고, 어," → dropped | better | Filler dropped. |
| 5:08 | 2 sentences in a cue → one sentence in 3 parts | better | — |
| 5:14 | "to get a Lama to 70B" → new transcript "Lama to 7 dB" → Korean "Llama 2 7B" | worse | Wrong term correction: the number became 7B; the old cue said 70B correctly. |
| 6:04, 6:11, 6:22 | rookie numbers; Chat-APT/CLAWD/BARD → ChatGPT/Claude/Bard | same | The old Korean already had the terms right. |
| 12:23 | "음" cue → dropped | better | A filler cue is gone. |
| 13:21, 13:27 | "mostly, mostly" | same | The old Korean had already dropped the repeat. |
| 16:28 | "저희는, 어, 이 과정을…" → "…이 과정을 파인 튜닝이라고 합니다." | better | Filler and false start dropped. |
| 17:32, 17:41 | "사전 학습 단계는, 음, 학습하는 것입니다," / "그것은, 어, 그러니까, 주는 것에 관한, 음, …" → clean sentences | better | The old Korean carried the fillers word for word. |
| 26:58 | "administer a lot of different tests to these large language models" → "예를 들어, 여러분은 다양한" | worse | Lost content: the title hint dropped "tests to these large language models", and the Korean cue breaks off. |
| 33:43, 33:55, 34:29, 34:38, 54:03, 54:29, 56:07 | same sentences, Korean already right | same | — |
| 34:18 | "trashabt" → "챗봇" / new "ChashDBT" → "ChatGPT" | better | Term fixed by the mishearings list. |
| 34:33, 34:41 | "그래서, 어," → dropped | better | — |
| 36:52 | "chong chong …" → "chunk, chunk, …" → "덩어리, 덩어리, 덩어리, 덩어리, 덩어리, 덩어리, 덩어리" | worse | The speaker's sound effect is now the noun "lump" seven times; the old "총총총…" read as a sound. |
| 36:57, 36:59 | one cue holding two sentences → one each | better | — |
| 42:39 | "그리고 음" → "그리고," | same | — |
| 45:03 | "클래스 시리즈나 Google의 바 시리즈" → "Claude 시리즈나 Google의 Bard 시리즈" | better | Terms fixed. |
| 45:11 | duplicated "빠르게 등장하고 성숙해지는" → one sentence in 4 parts | better | The old Korean repeated a clause. |
| 54:11, 54:15, 54:34 | "BART is hijacked", "marked down" → Bard, 마크다운 형식 | better | Terms fixed; one sentence per cue. |
| 56:27 | "luxe-leap 또는 에이전트 공격" → "백도어 에이전트 공격" | worse | Wrong term correction: the glossary turned the misheard "sleeper agent" into "backdoor agent". The old version was also wrong, but it did not assert a term. |
| 58:40 | "셸 브릭 공격", "백도어" → "탈옥 공격", "백도어 공격" | better | jailbreak was recovered from "shellbreak". |
| 0:00, 3:42, 7:34, 11:11, 14:47, 22:09, 32:39, 36:36, 39:58, 43:40, 50:26 | sampled; the same sentence is split into parts | same | Same meaning; the new parts are in Korean order. |
| 18:55 | "than your higher people" "상위 인력보다…" → "Then you hire people." "그런 다음 사람들을 고용합니다." | better | The new transcript heard the words right. |
| 24:52, 29:08, 54:20 | 2–4 old fragments → one sentence | better | — |
| 47:12 | "하지만 이것은 실제로 / 일어날 일입니다." → "하지만 실제로 이런 일은 일어나지 않을 것입니다." | better | The old cut inverted the meaning. |

## Regressions

Fixed during the task, before the final re-makes:

- A sentence carried to the end of a 26 s run-on: now ended at its last comma,
  or at the cap (`e3eec07`).
- A lone interjection rendered as a sentence: now "자," (`e3eec07`).
- A cut before a relative pronoun, which inverted the Korean (`e9f8b08`).
- "etc." read as a sentence end, and misheard spellings used as glossary terms
  (`3d0eaf9`).
- Korean over two phone lines (`2941558`).
- A failed re-make that left the new cues in place (`2c78179`).

Remaining, as read above. None was fixed after the final re-makes; a fix
would need another paid run of the video.

- Lost content:
  - `vSCYL8ySh9g` 12:37 and `zjkBMFhNj_g` 26:58: clauses the title hint dropped;
  - `WJpeW9lCSUM` 10:06 and `_MEqhFwnp8o` 8:41, 20:20: the new transcript differs there;
  - `WjhDDeZ7DvM` 2:44: the example "take you to" lost;
  - `_MEqhFwnp8o` 1:38: a dropped subject.
- Added content:
  - `vSCYL8ySh9g` 14:09: "중요하다고" ("it is important") completes a clause cut by the run-on limit;
  - `WjhDDeZ7DvM` 24:47: a spoken "Um," written as the title phrase.
- Changed meaning:
  - `WjhDDeZ7DvM` 13:55, 21:20;
  - `_MEqhFwnp8o` 20:52, 24:23, 25:27, 29:29, 39:56, 43:03;
  - `QDX-1M5Nj7s` 42:21;
  - `vSCYL8ySh9g` 11:40 (two sentences joined by a comma in the transcript);
  - `QWNxQIq0hMo` 2:16, 3:41;
  - `wbc-oxFFV9c` 32:15 ("U" heard as "you");
  - `zjkBMFhNj_g` 36:52 (a sound effect rendered as "덩어리" seven times).
- Content shown before or after its place inside a sentence:
  - `WjhDDeZ7DvM` 18:32–18:45 (a condition after its main clause);
  - `QDX-1M5Nj7s` 41:44 (a self-correction split).
  - No content moved across sentences in any place read.
- Wrong term corrections:
  - `WjhDDeZ7DvM` 19:22 (GPT-4Go → GPT-4, should be GPT-4o);
  - `zjkBMFhNj_g` 5:14 (70B → 7B), 56:27 (sleeper → backdoor agent);
  - `_MEqhFwnp8o` 2:48 (Chain of Thought), 17:34 (Nemotron Switchyard), 19:55 and 23:45 (names), 30:00 (COT);
  - `wbc-oxFFV9c` 40:07 (seq2seq → Sequence to Sequence twice), 44:09 (end token → N token);
  - `vSCYL8ySh9g` 14:33 (loop → latent recurrent transformer).
- Forced ends in unpunctuated stretches:
  - `QDX-1M5Nj7s` 0:09, 0:23, 0:36 (course name cut);
  - `wbc-oxFFV9c` 4:02, 4:04 (spoken arithmetic cut into fragments);
  - `QWNxQIq0hMo` 7:44, 12:37 (parts ending on "어쩌면", "끝 무렵에는");
  - `WjhDDeZ7DvM` 32:26–32:38 (a trailing fragment).
- Parts too short to read: `vSCYL8ySh9g` 8:35 (two one-word parts).
- Stray or duplicate cues from the pre-fix part of `WJpeW9lCSUM`: 1:16
  (shown twice), 1:43, 2:09, 2:58, 4:07.

## Reproducing

The scratch scripts are in `artifacts/sentence-cues/` (ignored by Git, as
they read the person's library):

- `metrics.py` counts the cue shape;
- `pairs.py`, `sel.py` and `compact.py` line up old and new for reading;
- `measure.test.svelte.ts` renders the line counts;
- `shots.test.svelte.ts` takes the before/after screenshots;
- `hintdiff.py` compares the transcripts with and without the hint.

The re-make tool is `src-tauri/examples/remake_saved.rs`.
