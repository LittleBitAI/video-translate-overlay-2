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

Changed after review, once the re-makes were done:

- A sentence whose reply keeps getting its parts wrong is asked again whole,
  never part by part; after three passes that save nothing the job stops
  (`translate_failed`).
- A question or exclamation mark ends a sentence, even before a lower-case
  word, unless it closes a quotation that carries on (`He asked "why?" and
  left.`). A period ends one even before a lower-case word, except after an
  abbreviation; an ellipsis, `etc.` or an initialism (`U.S.`) does only
  before a word that does not carry on in lower case.
- A sentence with a comma in it is no longer ended at the run-on limit: it
  goes whole to its sentence end, however long (decided by the person on
  2026-11-19, review round 2). Only a stretch with no punctuation is still
  ended at 210 characters.
- A cue never ends after its last word: a word that takes no time at the end
  of the audio joins the part before it, or is dropped if it is a sentence of
  its own.
- A re-translation asks for the same sentences as the first translation. A
  saved version that was received or selected again has its sentences rebuilt
  by running its cues' words through the same sentence ends. It also gets its
  mishearings found again: they are kept with a digest of the transcript they
  were found in, and used only while the cues hold that transcript.
- A re-make that fails keeps its draft for the re-make tool's `continue`.
  `continue` refuses a re-make that failed writing, whose transcript is
  unfinished, and one whose previous version was restored. While a re-make's
  draft is in the cue table, playback is given the selected saved version,
  not the draft.
- A write stopped with a sentence open also keeps that sentence's words as the
  first held piece, so a build from before sentence cues (0.1.9), which
  ignores the open sentence, resumes without losing them.

None of these changes the cues of the eight saved versions. In their cues no
part follows a `?` or `!`, no cue ends after the video, and no multi-part
sentence was ever answered part by part (the only replies with wrong ids, in
`wbc-oxFFV9c` and `WJpeW9lCSUM`, were to one-part sentences, and were asked
again). Re-translating a selected re-made version keeps the grouping its
re-make recorded. Only a version selected again or received from another
device has its grouping rebuilt with the new rules. Against the recorded
groupings, the rebuild joins 183 cues to the sentence before them (sentences
the run-on limit had ended at a comma) and splits 7 (a period before a
lower-case word):

| Video | Cues | Recorded parts | Rebuilt parts | Joined | Split |
| --- | --- | --- | --- | --- | --- |
| `QDX-1M5Nj7s` | 1,316 | 803 | 857 | 56 | 2 |
| `QWNxQIq0hMo` | 313 | 171 | 179 | 8 | 0 |
| `WJpeW9lCSUM` | 496 | 261 | 273 | 12 | 0 |
| `WjhDDeZ7DvM` | 770 | 426 | 449 | 23 | 0 |
| `_MEqhFwnp8o` | 1,008 | 444 | 478 | 34 | 0 |
| `vSCYL8ySh9g` | 314 | 182 | 188 | 6 | 0 |
| `wbc-oxFFV9c` | 1,490 | 536 | 538 | 7 | 5 |
| `zjkBMFhNj_g` | 1,409 | 740 | 777 | 37 | 0 |

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
| `WJpeW9lCSUM` (23:14), two re-makes | 122.99 | 139.25 + 127.55 | 389.79 | 387.2 |
| `WjhDDeZ7DvM` (39:57) | 143.56 | 199.18 | 342.74 | 665.8 |
| `_MEqhFwnp8o` (49:51) | 203.63 | 249.09 | 452.72 | 830.8 |
| `vSCYL8ySh9g` (15:04), two re-makes | 90.28 | 73.96 + 73.86 | 238.10 | 251.1 |
| `wbc-oxFFV9c` (1:16:12) | 462.01 | 502.81 + 0.99 + 1.02 | 966.84 | 1,270.0 |
| `zjkBMFhNj_g` (59:48) | 373.09 | 408.01 | 781.10 | 996.7 |

All amounts are in KRW. Paid Gemini spending for the task was 2,280.06 KRW:
55.66 KRW of trials on short excerpts and 2,224.40 KRW of re-makes. The
re-makes cost 290 to 410 KRW per hour of video. The last column is the old
1,000 KRW per hour target. Seven videos stayed under it, including what they
had spent before. `WJpeW9lCSUM` ended 2.6 KRW over it after its second
re-make. That re-make ran after the person removed the per-hour stop from the
app and the spending ceilings from this task (`ed008fc`).

- Dory, the 15-minute DeepSeek video and `WJpeW9lCSUM` were made a second
  time, as the person approved, after the run-on fix (`e3eec07`). Their first
  re-makes had windows written before that fix. The first re-make of each
  stays as a saved version, so these three have three saved versions.
- wbc's first run ended `translate_failed` with one cue untranslated (cue
  1388, 70:46). Gemini answered that one-part sentence with two segments
  (ids 1388 and an invented 1389), which the id check refused. The tool's
  rollback had done nothing, because selecting the version that is already
  selected keeps the cue table. That was fixed (`2c78179`). The job was then
  continued twice for the one cue; the second reply matched. A restore between
  the two left a newer job row for the old version, so the installed app showed
  the old cost. The tool now makes a continued re-make the newest job, and
  that row was repaired in the library.

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
| `WJpeW9lCSUM` Nemotron Bolt 2 Bit (23 min) | 309 → 496 | 175 → 273 | 261 / 12 | 171 → 258 | 2 → 0 | 41 → 2 | 0 → 0 | 1 → 0 | 3 → 1 | 0 → 0 |
| `WjhDDeZ7DvM` Build DeepSeek: Series Introduction (40 min) | 408 → 770 | 167 → 458 | 426 / 32 | 164 → 434 | 14 → 0 | 86 → 2 | 2 → 3 | 4 → 2 | 3 → 0 | 1 → 0 |
| `_MEqhFwnp8o` Ask the Experts: Dory (50 min) | 678 → 1,008 | 398 → 485 | 444 / 40 | 394 → 453 | 17 → 0 | 78 → 0 | 139 → 7 | 50 → 10 | 60 → 0 | 4 → 0 |
| `vSCYL8ySh9g` Dory - looped transformer (15 min) | 202 → 314 | 106 → 188 | 182 / 6 | 103 → 176 | 7 → 0 | 22 → 0 | 0 → 0 | 3 → 3 | 0 → 0 | 0 → 0 |
| `wbc-oxFFV9c` Give Me 1 Hour, Deep Learning (76 min) | 885 → 1,490 | 307 → 540 | 536 / 8 | 311 → 520 | 91 → 1 | 202 → 2 | 0 → 0 | 5 → 3 | 1 → 1 | 1 → 1 |
| `zjkBMFhNj_g` Intro to Large Language Models (60 min) | 892 → 1,409 | 375 → 781 | 740 / 41 | 366 → 745 | 53 → 0 | 97 → 0 | 8 → 1 | 3 → 3 | 17 → 0 | 0 → 1 |
| All 8 | 4,293 → 7,116 | 1,883 → 3,794 | 3,563 / 237 | 1,868 → 3,628 | 239 → 5 | 657 → 7 | 150 → 12 | 79 → 39 | 90 → 2 | 7 → 2 |

Reading the table:

- Most cues that do not end at a sentence end, or start mid-sentence, are
  now parts of a split sentence. 3,563 of the 3,794 are such parts.
  - Their sentence was translated in one request, so the Korean of each part
    is in Korean order.
  - In the old version, every such cut was translated alone.
  - The rest, 237, are forced ends in unpunctuated stretches; QDX has 86 of
    them.
- Cues holding two sentences fell from 239 to 5. None of the 5 holds two real
  sentences:
  - three are periods the speech model wrote before a lower-case word (QDX
    6:09, 13:52, 35:40);
  - two are abbreviations ("Dr.", "U.S.").
- The English fillers and repeats fell because the new transcripts write fewer
  of them, not because they were removed. The saved English is kept as heard.
- In the Korean, fillers fell from 90 to 2:
  - `'cat'에` (wbc 27:28) is a false match;
  - "음," for a spoken "Well," (`WJpeW9lCSUM` 12:23) is a filler kept, listed below.
- Repeats in the Korean fell from 7 to 2:
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
| Page overlay | windowed PC 1280×800 | 2,064 / 2,092 / 137 (up to 7) | 6,828 / 288 / 0 |
| Page overlay | fullscreen PC 1920×1080 | 2,098 / 2,061 / 134 (up to 7) | 6,845 / 271 / 0 |
| Page overlay | phone 384×854 | 1,261 / 2,396 / 636 (up to 9) | 5,582 / 1,534 / 0 |
| Watch screen | windowed PC 1280×800 | 2,046 / 2,105 / 142 (up to 7) | 6,817 / 299 / 0 |
| Watch screen | fullscreen PC 1920×1080 | 2,089 / 2,070 / 134 (up to 7) | 6,838 / 278 / 0 |
| Watch screen | phone 384×854 | 997 / 2,210 / 1,086 (up to 10) | 4,791 / 2,313 / 12 |

Per video, the cues on more than 2 lines, old → new:

| Video | Overlay, windowed | Overlay, fullscreen | Overlay, phone | Watch, windowed | Watch, fullscreen | Watch, phone |
| --- | --- | --- | --- | --- | --- | --- |
| `QDX-1M5Nj7s` | 41 → 0 | 41 → 0 | 102 → 0 | 43 → 0 | 41 → 0 | 156 → 1 |
| `QWNxQIq0hMo` | 15 → 0 | 15 → 0 | 37 → 0 | 15 → 0 | 15 → 0 | 57 → 2 |
| `WJpeW9lCSUM` | 8 → 0 | 6 → 0 | 34 → 0 | 8 → 0 | 6 → 0 | 71 → 3 |
| `WjhDDeZ7DvM` | 24 → 0 | 23 → 0 | 74 → 0 | 24 → 0 | 23 → 0 | 124 → 0 |
| `_MEqhFwnp8o` | 12 → 0 | 12 → 0 | 73 → 0 | 12 → 0 | 12 → 0 | 135 → 2 |
| `vSCYL8ySh9g` | 1 → 0 | 1 → 0 | 24 → 0 | 1 → 0 | 1 → 0 | 45 → 0 |
| `wbc-oxFFV9c` | 18 → 0 | 18 → 0 | 187 → 0 | 19 → 0 | 18 → 0 | 312 → 3 |
| `zjkBMFhNj_g` | 18 → 0 | 18 → 0 | 105 → 0 | 20 → 0 | 18 → 0 | 186 → 1 |
| All 8 | 137 → 0 | 134 → 0 | 636 → 0 | 142 → 0 | 134 → 0 | 1,086 → 12 |

Exceptions: 12 new cues take three lines in the Watch screen at phone size,
and in no other place. Its player is 352 px wide, and its subtitle font fits
about 20 Korean characters a line. These 12 cues have 47 to 71 Korean
characters with few break points. The 70-character English cap does not bound
the Korean, which is sometimes longer than the cap assumed. The longest,
`WJpeW9lCSUM` 226, adds the English term in parentheses after its Korean name.

- `QDX-1M5Nj7s` 1271
- `QWNxQIq0hMo` 87, 221
- `WJpeW9lCSUM` 141, 226, 291
- `_MEqhFwnp8o` 86, 906 (an e-mail address)
- `wbc-oxFFV9c` 227, 484, 1317
- `zjkBMFhNj_g` 1224

In the page overlay at phone size, these cues fit two lines.

The measurement is
[measure/measure.test.svelte.ts](measure/measure.test.svelte.ts). It renders
every cue and asserts the count on each number of lines, and the list of cues
over two lines, for every surface, size, version and video, against
[measure/expected.json](measure/expected.json). It also asserts the totals for
the new versions given above. The tables above are sums of that file, which
[measure/check_readme.py](measure/check_readme.py) checks. Its input, every
Korean cue of the 16 versions, stays out of Git as the person's subtitles. The
run's command, output and the input's SHA-256 are in
[receipts.md](receipts.md), with the receipts of the tests named in this
document.

## The installed app (done item 16)

The installed Windows app is version 0.1.9. It opened `wbc-oxFFV9c` from its
Library, twice. [receipts.md](receipts.md) holds the second time: the app's
file version and hash, the library's hash and selected version, and the app's
own command output. Its overlay showed:

- the new first cue ("1시간만 투자하세요, …"), with "자막 준비됨 · 966.8원";
- during playback, the new cue at 16:59 ("정확히 얼마나 적을까요?"; the old one was "얼마나 적을까요?"), and on the second opening at 17:01 ("이제 코드로 16개의 필터를 만들고 개수를 세어 봅시다."; the old one was "코드로 16개의 필터를 만들어서 세어 봅시다.").

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
Each place is one new sentence with the old cues that overlap it. The old and
new Korean of every place, with its verdict, are in [places.md](places.md); the
tables below give the verdicts and reasons, grouping places that share one. A
place given as a range spans several new sentences, each a row there. The
places are:

- every place where the old version had a filler or an immediate repeat;
- every place where the old version had a term the glossary lists as misheard;
- at least 20 places per video in all, spread evenly, most of them where the
  old version cut a sentence.

The person narrowed this on 2026-11-19. Mid-sentence cuts are covered in full
by the counts above, and by sampling here.

After review round 2, every place has a reason of its own. 65 places had none
("—") and 32 had only "Same." or "Same meaning.". Each was read again from its
pair in [places.md](places.md). Four Dory places read again turned out
better, not same: 4:28, 9:50, 23:25 and 26:19, where the old version cut a
phrase across cues. One `vSCYL8ySh9g` row at 9:44 covered two sentences and
is now two places.

| Video | Places read | Better | Same | Worse |
| --- | ---: | ---: | ---: | ---: |
| `QDX-1M5Nj7s` | 38 | 17 | 16 | 5 |
| `QWNxQIq0hMo` | 23 | 11 | 8 | 4 |
| `WJpeW9lCSUM` | 33 | 15 | 11 | 7 |
| `WjhDDeZ7DvM` | 75 | 43 | 23 | 9 |
| `_MEqhFwnp8o` | 263 | 204 | 44 | 15 |
| `vSCYL8ySh9g` | 29 | 13 | 11 | 5 |
| `wbc-oxFFV9c` | 60 | 26 | 29 | 5 |
| `zjkBMFhNj_g` | 57 | 25 | 28 | 4 |
| All 8 | 578 | 354 | 170 | 54 |

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
| 28:49 | "all the deep neural network is is" → "깊은 신경망이란 그저…" | same | Both say a deep network is layers stacked on layers. The new keeps the speaker's restart ("깊은 신경망을… 깊은 신경망이란 그저") but nothing is lost or added; the old second line belongs to the next sentence. |
| 32:04, 32:09, 32:12 | one 4-line run-on with "right" ×2 → 3 sentences | better | Tics dropped. |
| 42:10 | 5 old fragments → 4 parts | better | The old spread the back-propagation sentence over five cues, with "맞습니다" and "그렇죠?" tics and the weights stranded in a later cue. The new is one sentence in four parts ending on "the weights we care about", tics dropped. |
| 42:21 | "…also with W1. That's why we really call it back propagation" → "이 정보를 활용하기 때문에 이를 역전파라고 부르는 것입니다." | worse | The new transcript dropped "also with W1", and the Korean turns "that's why" into "because it uses this information": changed meaning. |
| 42:34, 42:46, 42:57 | one cue of 7+ lines → 3 sentences in parts | better | The old cue could not be read in its time. |
| 43:41 | "very, very deep" → "매우, ‖ 깊은" | same | The cut falls inside the repeat, leaving "매우," at the end of a part. |
| 51:17 | old "…우리는 분할할 수 있습니다 / 배치를…" → one sentence | better | The old ended a cue on "we can split" with its object "the batch" in the next cue. The new says it in one sentence: split the batch across workers for a bigger speed-up with GPUs. |
| 51:26 | "very, very" → "아주"; old fragments → one sentence | better | The old cut "the last topic is this / the topic of overfitting" across two cues. The new is one sentence, "the last topic I want to cover very briefly is overfitting", with the repeat "very, very" said once. |
| 41:44 | "w2가 아니라, 죄송합니다, w1이 됩니다" → "경우에는 w2가 아니라 w1, ‖ 아니, w1이 됩니다." | worse | A forced end inside an unpunctuated stretch splits the speaker's self-correction ("not w1, but w, excuse me, not w2, but w1"), and the Korean garbles it. |
| 1:01 | old fragments → one sentence in parts | better | The old broke "problems we never thought we could solve" over three cues and said "생각" twice. The new is one sentence in five parts, said once. |
| 9:12 | old cue repeating "금요일" → one sentence in parts | better | The old said "금요일 … 다음 단계" in two cues in a row. The new says it once, in one sentence about the project pitch competition. |
| 45:11 | old fragments → one sentence in parts | better | The old split "too small and / too large" across cues and left "최솟값!" as a cue of its own. The new keeps the comparison in one sentence ending on the value large enough to pass local minima. |
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
| 0:04 | greeting | same | The same welcome; "만들기" → "구현하기" for "from scratch". |
| 0:58 | 1 cue → 3 parts | same | The same sentence (researchers, engineers and companies moving from OpenAI to DeepSeek calls). The old cue also held the next sentence. |
| 4:23 | same sentence | same | The same wish that you enjoy the series, worded differently. |
| 6:11 | 1 cue → 3 parts | same | The same sentence; the new puts the reaction verb at the end as Korean does, in three parts. |
| 8:37 | 1 cue → 3 parts | same | The same sentence (huge view counts, all 10 or 15 minutes long). The old cue also held the next sentence about an 8-minute video. |
| 13:32 | same sentence | same | The same sentence about the team's time and effort. The old cue also held the sentence before it ("I spent a lot of time compiling this"). |
| 2:16 | → "…그 영상들에서 저는 여러분께 ‖ 모든 구성 요소를 하나하나 가르쳐 드릴 것입니다. ‖ DeepSeek 아키텍처의 가르침이란," | worse | Changed meaning. The run-on limit ended the sentence at "and by teach,". Gemini moved "of the DeepSeek architecture" into the last part and made it "the teaching of the DeepSeek architecture is". |
| 3:41 | "이 재생 목록을 모두 마치고 나면 말이죠." → "이 과정을 모두 마친 후에 말이죠." / "전체 재생 목록입니다." | worse | The cap cut "this / entire playlist" inside the noun phrase. Gemini translated the last part as a sentence of its own ("it is the entire playlist"). |
| 5:17 | old "그 이유는 바로 /" → one sentence | better | The old left "the reason is" alone in a cue. The new joins it to its clause: the losses came because DeepSeek built a reasoning model that competes with OpenAI's best. |
| 7:05 | old cut mid-sentence → one sentence | better | The old split "R1 was not the first / from this company" across cues, with the next sentence's start. The new is one sentence: R1 was not the first thing this company made. |
| 9:40 | old cut mid-sentence → one sentence | better | The old cue ran into the next sentence ("다시 인터넷을 검색해 보니 구축"). The new holds this sentence alone: the real power is here, not in building on what exists. |
| 11:02 | old duplicated "가르치기 위한 계획을 세웠습니다" → one sentence | better | The old said "made a plan to teach" twice in two cues. The new says it once. |
| 11:53 | old cut mid-sentence → one sentence | better | The old cue ran into the next sentence ("MLA 부분을 살펴보면 이런 종류의"). The new is just "I can show you some of my Canva notes". |
| 7:44 | four old sentences → "이는 정말 모두를 놀라게 했는데 ‖ … ‖ 모델이었기 때문이며, 이는 2025년 1월에야 발표되었으니 ‖ 1년 정도의 연구가 있었거나 어쩌면" | worse | The new transcript has "R1. which" (a period before a lower-case word), so three sentences run together, and the run-on limit ends the last part on "어쩌면" ("or maybe"). |
| 9:07 | "…짚어주는 영상 말이죠, 그렇죠?" → "…짚어주는 영상 말입니다." | better | The tic "right?" is dropped. |
| 12:37 | → "…비로소 우리는 어떻게 모든 ‖ DeepSeek의 요소가 처음부터 조립되는지 알게 될 것이고 ‖ 이 과정의 끝 무렵에는" | worse | A run-on stretch. "모든" ("every") is cut away from its noun across parts, and the sentence ends on "이 과정의 끝 무렵에는" ("towards the end of this"). |

### WJpeW9lCSUM — NVIDIA's Nemotron Bolt 74B-A9B 2 Bit (talk, single speaker)

Old 309 cues, new 496 cues in 235 sentences. Mishearings: GDUF → GGUF and unslothed → Unsloth. This video was re-made a second time, after the run-on fix, because 34 of its windows in the first re-make predated that fix. The reading below is of the second re-make.

I read 33 places: both old filler places, all 5 term places, 20 spread evenly, and the 6 places where the first re-make was worse. Tally: better 15, same 11, worse 7.

| At | Old → new (Korean) | Verdict | Why |
|---|---|---|---|
| 0:00 | "경험칙은 꽤" / "간단했습니다. 4-bit로…" → "자, 오랫동안 …경험 법칙은" / "꽤 간단했습니다." | better | The old cue ended mid-word-group, and the next one carried two sentences. |
| 1:16 | "It's a mixture of experts models." at 76.7 s for 1.7 s and again at 78.4 s for 0.28 s | worse | The sentence is shown twice. Both copies come from one writing window: whisper repeated the window's last sentence in 280 ms. Remaining. |
| 1:35 | "…24GB 장치 하나에서 로컬로 실행할 수 있습니다." → "핵심은" / "이 모델이 기본적으로" / "로컬에서 실행 가능한 에이전트 모델이며" / "24GB 장치 하나에서 다른 앱과 함께 구동할 수 있다는 것입니다." | better | The first re-make's stray cue "your other apps.", with its repeated verb, is gone. |
| 2:03 | "…지도를 손에 넣은 셈입니다." / "…모든 기술에 대한 지도를 갖게 된 셈입니다." → "…모든 기법에 대한 지도를 얻게 된 셈입니다." / "요즘 사람들이 더 작은 모델을 만들기 위해 사용하는." | worse | The first re-make's one-word cue "날들입니다." is gone. But the Korean moved the head noun into the first part, so the second part is a modifier with nothing to modify. Remaining. |
| 2:17 | orphan "Hugging Face에서 보기 시작한 2-bit 범위입니다." → 4 parts ending "2비트 범위의 Hugging Face 모델들 말이죠." | better | The Korean moved "이제 막 보이기 시작한" one part earlier, which stays inside the sentence. |
| 2:53 | "영상이 올라올 때마다…" → "좋아요와 구독을 눌러주시면" / "더 많은 사람들에게 영상이 노출되어" / "영상이 나올 때마다 바로 확인하실 수 있을 겁니다." | better | The first re-make's one-word cue "나옵니다." is gone. |
| 3:01 | GDUF; one 4-line cue with "(PTQ)" → 3 parts | better | The term is correct in the Korean, as before. The added parenthetical is gone, and each cue fits two lines. |
| 3:11 | orphan "더 적은 비트로요." → "…모든 가중치를 더 적은 비트로 반올림한 것입니다." | better | The fragment is gone. |
| 3:20 | "…가장 흔한 방법들은" / "nvfp4가 있고, …" → "아마도 현재 가장 흔한 방식은 NVFP4일 것입니다." | better | One sentence, and the term is capitalised. |
| 3:24 | "unslothed 모델들도 있으며" → "Unsloth 모델도 있습니다." | better | The mishearing is corrected. The old Korean passed "unslothed" through. |
| 3:26 | "…다른 방식들도 있습니다" / "또한 인기를 얻고 있습니다" → "그 외에도 인기를 얻고 있는 몇 가지 모델이 더 있습니다." | better | The old cut left a sentence with no subject. |
| 3:46 | same sentence, same Korean | same | The same sentence and meaning (4-bit costs almost nothing on most models). The old cue also held the next sentence, which now has its own. |
| 4:07 | "where you see a huge / amount of deterioration." → "상전이 구간에서 큰 차이가 나타납니다." / "성능 저하가 크게 나타납니다." | worse | The Korean says the change twice, once per part, and "차이" is not in the English. Remaining. |
| 4:54 | "(PTQ)" added, 2 cues → 3 parts | better | No added content; the parts follow the Korean order. |
| 6:00 | same sentence, same Korean | same | Identical Korean. |
| 7:04 | same sentence, same Korean | same | The same Korean ("분명히" → "분명"). The old cue also held the next sentence about the 2-bit MoE numbers, which now has its own. |
| 8:41 | same sentence, same Korean | same | The same sentence without the subject "그들은", which Korean leaves out anyway. The next sentence, which the old cue held, is now its own. |
| 11:52 | same sentence, same Korean | same | Same meaning. The new drops the acronym "QAD" the old added in brackets; the English here says only "quantize-aware distillation", so nothing said is lost. |
| 12:57 | same sentence, same Korean | same | Same meaning ("트레이드오프" → "절충안"), now in two parts. |
| 14:04 | same sentence, same Korean | same | Identical but "1.58-bit" → "1.58비트". |
| 7:52 | "Qwen 3.6이나 Gemma 4 모델보다 뒤처져 있죠." → "Nemotron 3.6과 Gemma 4 모델보다 뒤처지는 성능입니다." | worse | Wrong term: whisper wrote "QN 3.6", and the translator made it the video's own model instead of Qwen. The old Korean was right. Remaining. |
| 9:46 | "양자화 인식 학습 (QAT)이 등장하며…" → "이 단계에서 바로 QAT가 도입됩니다." | same | The term is kept; the rest of the old cue is the next sentence. |
| 10:06 | "업데이트를 위해서요. 그래서…" → "가중치의 전체 정밀도 사본은 유지해야 합니다..." / "그래서 모델 자체가…" | worse | Lost content: "for the updates" is not in the new transcript, which trails off at "for...". Remaining. |
| 10:37 | "…이 작업에서 흥미로운 부분입니다." / "영상에서…" → "두 가지 모바일 버전이" / "사실 이번 영상에서 흥미로운 부분입니다." | better | The old cut split "this / video" across two cues. |
| 15:19 | 3 cues of a run-on → "코드인지 텍스트인지에 따라," / "초당 500에서 600 토큰 정도의 속도가 나옵니다." | better | One sentence, nothing cut. |
| 16:22 | "…이건 확실히" → "여기 다른 실행에서 나온 결과가 하나 있습니다." | better | The old cue ended mid-clause. |
| 17:13 | same meaning | same | The new sentence is only "you'll see that, sure enough, it's going through" ("보시다시피 확실히 잘 작동하고 있습니다"), which is right. The faster token speeds that the old cue added belong to the next sentence. |
| 18:30 | same meaning | same | Same trailing-off phrase ("Now, with the pro calls..."); "pro" became "프로". |
| 18:32 | "…약간 혼동할 수 있다는 것을 발견했습니다." → "가끔" / "약간 혼동하는 경우가 있는데," / "여기 있는 프롬프트 주입 저항 같은 특정 항목들에서 그렇습니다." | same | Both read well; the new drops "I find", which carries no content. |
| 18:42 | "all all eight" → "때때로 여기서 8개 중 8개를 모두 맞히는 것을 볼 수 있습니다." | better | The repeat is dropped. |
| 19:21 | 4 cues → 5 parts of one sentence | better | The old Korean ended a cue on "알 수 있습니다," and started the next with a new claim. |
| 20:48 | "…3090이나 4090 같은 것을 가지고 계시다면," → "RTX Pro 6000이나 심지어 3090 정도만 가지고 있어도 충분하다는 것입니다." | worse | Changed meaning. The new transcript ends a sentence after "3090,", and the Korean closes it with "is enough". The speaker says owners of these cards are better off with a bigger model. Remaining. |
| 21:02 | "3090이나 4090" → "495090을 사용하신다면," | worse | The mishearing "495090" is passed through. The old Korean said 4090, and this run's glossary did not list it. Remaining. |

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
| 22:44 | one 4-sentence cue → 3 parts | better | The old cue held four sentences at once (the GPT-4o advice, the startup advice, the cost, open source). The new shows this sentence alone in three parts: a small, fast-growing startup should pick DeepSeek for the cost; it performs well and is open source. |
| 23:18 | GPT-40 → GPT-4o in 2 parts | same | Already correct. |
| 24:47 | "Um," / "음," → "DeepSeek from Scratch" / "DeepSeek를 처음부터 구현하면" | worse | The title hint wrote title words that were not said, and the Korean adds content. |
| 26:19 | "which which … 어떤 부분이 결정하는" → 3 parts | better | Repeat dropped; the old Korean was garbled. |
| 28:28 | 2 cues → 2 sentences | better | The old cut "instead of encoding it in the token / instead of in the embedding itself" across cues, saying "대신" twice. The new is one sentence in two parts: encode position at the query and key stage, not in the token embedding. |
| 28:38 | "rotary … which which is the fifth key innovation which / The DeepSeek architecture implemented." | same | The source itself breaks the clause; the Korean "…관한 것으로," / "DeepSeek 아키텍처에 구현되었습니다." keeps the meaning. |
| 29:41 | 3 parts | better | The old cue began with the previous sentence about the RL training mechanism. The new holds this sentence alone in three parts: this is why DeepSeek R1 is such a good reasoning model. |
| 32:26–32:38 | "…Llama 등과 비교했을 때 말이죠." / "the cost requirements which they use to train the models." → "그들이 모델을 학습시키는 데 사용하는 비용 요건입니다." | worse | The run-on limit ended the sentence at its last comma, so the closing phrase stands alone as a fragment. |
| 32:41 | "So it's like..." → "그러니까," | same | A phrase the speaker trails off on. Both render it as "그러니까", the old at the end of the cost sentence, the new on its own. |
| 35:01 | "you you go through" → "이전 시리즈를 먼저 학습했다고 가정합니다." | better | Repeat dropped. |
| 35:07, 35:13 | "phase three gpu / optimization text" → "3단계는 GPU 최적화입니다." | better | The misheard "text" was dropped from the Korean, and there are no orphans. |
| 38:16 | GRPO in 2 parts | same | The run-on limit ended the sentence before "which is also known as…", so this part went alone. Its Korean still says GRPO is the reward used instead of human-labelled data, so nothing is lost; the term is split across the two lines. |
| 38:22 | PTA → PTX | better | Mishearing corrected; the expansion "Parallel Thread Execution" is not repeated in the Korean (minor). |
| 0:04 | "여러분 안녕하세요," / "'Build DeepSeek from Scratch' 시리즈의…" | same | Said in the audio; the series name is left in English. |
| 2:44 | "your effort will take you 2." "당신을 2로 이끌 것입니다" → "여러분의 수고는 결실을 맺을 것입니다." | worse | Lost content: the example is about predicting the next token ("to"), and the new Korean drops it. |
| 5:14, 16:53, 24:55, 30:23, 32:56 | long cues → 2–3 parts | better | Korean order; no fragments. |
| 7:48 | sentence split, same content | same | The same opening ("you've all used ChatGPT"). The old cue also held the next sentence about early models and e-mails. |
| 10:00 | sentence split, same content | same | Same meaning; "fine-tune" → "미세 조정", "라벨링된" → "레이블이 지정된". |
| 14:51 | sentence split, same content | same | The new transcript has a period after "LLMs", so "the different LLMs DeepSeek built" is the next sentence. The Korean of each is right. |
| 22:17 | sentence split, same content | same | Same content in three parts. The new transcript heard "671 billion" where the old had "670" (DeepSeek-V3 has 671B), and the Korean follows it. |
| 27:16 | sentence split, same content | same | The same sentence ("the best way to understand quantization is to look here"). The old cue also held the sentence before. |
| 37:06 | sentence split, same content | same | The same heading, "strengths and weaknesses". The old cue also held the next sentence, now on its own. |

### _MEqhFwnp8o — Ask the Experts: Dory Looped Transformer Model (live stream, four speakers)

Old 678 cues, new 1,008 cues in 564 sentences. Mishearings: "loopy boys" → Looped Transformer (wrong: that is the host's joke name, though the Korean did not use it), "Star Elastics" → Star Elastic, "4B model" → 4B. I read 263 places: every old filler or repeat place (239), every misheard-term place (12, marks F/T), and 16 spread evenly. Tally: better 204, same 44, worse 15.

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
| 33:24 MT | same | The new transcript puts "It's a 4B model." alone; "이것은 4B 모델입니다" says it. The rest of the old cue (try it yourself, rent an instance) is the sentences around it. |
| 33:25 FMT, 33:29 FM, 33:30 FM, 33:34 FM, 34:08 FM, 34:12 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 34:25 FM | same | The source is garbled; both Korean versions are hard to read. |
| 34:45 MT | same | The same content (start at 0.5B or 1B, or go up to 4B as a proof of concept) and the tic "그렇죠?" is gone, but the new leaves "어느 정도," as a part of its own. One gain, one loss. |
| 35:01 MT | same | The same sentence (a quantized 4B model runs on a phone, a laptop and more), in two parts instead of two cues. |
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
| 47:56 MT | same | The same example (a regular 4B model such as Dory), with "Dorie" written as Dory in both Korean versions. The old third cue ("마치") belongs to the next sentence. |
| 48:54 FM, 48:55 FM, 48:56 FM, 48:57 FM, 49:00 FM, 49:03 FM, 49:07 FM, 49:10 FM, 49:14 FM, 49:18 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 49:23 FM | same | The run-on limit cut "next / week" across two sentences, so the Korean says "won't see you" in both. |
| 49:34 FM | better | Fillers, false starts or repeats are dropped from the Korean with the meaning kept, and each cue is one sentence or a part of one. |
| 49:44 FM | same | The same farewell; the new leaves out the Thanksgiving break the old included, which the new transcript has as the sentence before. |
| 1:01 M | same | The new transcript ends a sentence after "Hi everyone"; the welcome to the Nemotron live stream follows as its own. |
| 4:28 M | better | The old cut "each / layer" across two cues. The new has the clause whole: each layer sees this token only once. |
| 6:41 M | same | A stray "directions." cue, as in the old version ("방향들을요"). |
| 9:50 M | better | The old cut "for example / at the question level" across two cues. The new is one sentence in two parts: per question or even per token. |
| 11:52 M | same | The same sentence (not always a test, just a leftover of how we talk), one sentence in two parts instead of two cues. |
| 13:53 M | same | The same statement that the final response's chain of thought stays visible. The old ended on the question "is that right?", which the new transcript has as the next sentence. |
| 16:05 M | same | The same sentence about the fourth loop refining on top. The old cue also held the sentence before it about the third loop. |
| 19:20 M | same | "Here we go." in both ("여기 있습니다" → "됐습니다"). The old cue also held the start of the next sentence. |
| 23:25 M | better | The old spread this sentence over three cues that each began or ended mid-clause ("우리는 아마도 / 개선이"). The new is one sentence in four parts, with the reason last. |
| 26:19 M | better | The old cut the condition ("if you want absolute maximum performance") from its consequence with a period between cues. The new joins them: if you want it, you start from scratch and ignore the training cost. |
| 31:45 M | better | The false start was dropped; one sentence. |
| 35:21  | same | "You have to start somewhere." in both. The old cue also held the next sentence. |
| 38:44 M | same | The same sentence about two extreme experiments. The old cue also held the first experiment, which the new has as the next sentence. |
| 40:46  | same | The transcript differs here ("Is it available? In build.nvidia.com"); I can't verify which is right. |
| 44:02 M | same | The same answer: Dory is a looped transformer based on dense. The old put the question and "familiar dense patterns" in the same cues; the new has them as the sentences around it. |
| 46:25  | same | Identical Korean. |

### vSCYL8ySh9g — NVIDIA's looped 4B model (talk, single speaker; re-made twice)

The first re-make was replaced by a second one on the run-on fix, as approved. This reading is of the second. Old 202 cues, new 314 cues in 132 sentences. Mishearings: DOI → Dory, vFloat16 → bfloat16. I read 29 places:

- every old filler place (5);
- every place marked as a misheard term (7). One is real (DOI at 6:14); in the other six my term check matched "DOI" inside "doing";
- the old cues that started or ended mid-sentence, spread evenly (16).

Tally: better 13, same 11, worse 5.

| At | Old → new (Korean) | Verdict | Why |
|---|---|---|---|
| 4:20 | "…서로 다른 가중치 세트가 아니라 / 각 블록마다 매번, 기본적으로 동일한 파라미터 세트가 그저 / 여기서 강화됩니다." → "…그 수정 사항이 ‖ 매번 각 블록마다 서로 다른 가중치 집합을 사용하는 대신, ‖ 기본적으로 동일한 가중치 집합이 ‖ 여기서 강화된다는 것입니다." | better | One sentence; the verb is in the last part. |
| 6:14 | DOI → Dory | same | The old Korean already said Dory. |
| 9:44 | "Okay." → "자," | same | The new transcript puts "Okay." alone; "자," says it as the old "네," did. |
| 9:44 | "네, 그래서 이 부분이 저에게는, / 가장 궁금했던 부분입니다." → "그래서 이 부분이 ‖ 제가 가장 궁금했던 점인데, ‖ …" | same | Same meaning: the part he was most curious about, what the model does on each loop. The old took four cues, the new one sentence in four parts. |
| 12:04, 12:07 | → "이 방식은 전혀 효과가 없으며…" / "아마도 그 이유는 ‖ 모델의 가중치가 충분하지 않기 때문일 것입니다." / "26B 버전과 비교하면 말이죠." | same | "Compared to the 26B version" is now its own short sentence; nothing lost. |
| 12:37 | "따라서 이들의 연산량은 대략 비슷합니다." → "이 모델들도 마찬가지인데," | worse | Lost content: the new transcript has "So the same for these", without "the compute is roughly sort of". The first re-make, without the run-on fix, lost it too. It came from the title hint: the hint-off transcript of the same audio keeps it. |
| 13:17 | GPT-6 rumour, 1 → 3 parts | same | Same content: rumours that GPT-6 uses one, two or three loops by size. The new has it in three parts, with "아마도" for "supposedly". |
| 13:25 | two-loop default, 1 → 3 parts | same | Same content: deploy with two loops by default, as NVIDIA recommends. Three parts in the new, one cue in the old. |
| 14:03 | "…저는 이런 종류의 연구용 버전이 / 사람들이 프로덕션에서…" → one sentence in 3 parts | better | The old cut the sentence after its subject. |
| 14:09 | → "하지만 이런 종류의 모델을 미세 조정하는 방법을 살펴볼 때 ‖ 다양한 루프 횟수 등에 맞춰 조정하는 것은 중요하다고 생각합니다." | worse | Content added. The run-on limit ended the sentence at "etc.," before "you could imagine…". Gemini completed the cut clause with "중요하다고" ("it is important"), which the speaker did not say. |
| 14:29 | → "어쨌든 모델 카드에 대한 링크를 ‖ 설명란에 모두 적어 두었습니다." | same | "And all the details" is folded into "모두" ("all"). |
| 14:33 | "루프 트랜스포머" → "잠재 순환 트랜스포머" | worse | Wrong term: "loop transformers" became "latent recurrent transformers" (glossary term). |
| 0:00, 0:56, 2:24, 3:17, 4:11, 5:13 | old cues cut mid-sentence ("…서로 다른 양의 루핑이 일어나고 있는 것입니다. / 모델입니다.", "open mdw 라이선스로 공개했습니다. / 라이선스이므로…") → one sentence or one part per cue | better | The old repeated or orphaned words at each cut. At 2:24 OpenMDW is now one name. |
| 1:44 | same sentence | same | Same sentence ("나온" → "개발한"). |
| 6:02 | same sentence | same | Same sentence. The new says "112층" ("112 layers"), which the talk is counting. |
| 12:59 | same sentence | same | Same sentence. The old cue held "…and then being able to compete with", the start of the next sentence; the new ends where the transcript's sentence ends. |
| 6:52 | "…마지막 루프는 변하지 않습니다 / 전혀 변하지 않습니다." → one sentence | better | The old said "does not change" twice. |
| 7:47 | old fragments → one sentence | better | The old spread the point over three cues ("이것이 바로 / 현재로서는 이 부분이 /"). The new is one sentence in four parts: you must set the loop count when starting vLLM. |
| 9:25 | old duplicated clause → one sentence | better | The old said "if I run the same with a for loop" twice in two cues. The new says it once, with its result. |
| 10:07 | old fragments → one sentence | better | The old split the purpose ("match vLLM's log probs / and to see how the outputs change") across cues. The new is one sentence in two parts. |
| 10:55 | old fragments → one sentence | better | The old cue ran "at each point" into the next sentence. The new is one sentence: run a separate vLLM instance at each point. |
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
| 40:04 | "여기 첫 번째 답변이 있습니다." | same | The same sentence. The old cue also held "Number 9…", now its own sentence. |
| 40:06 | "9번입니다." | same | The transcript ends a sentence after "Number 9."; "9번입니다." says it. Seq2Seq follows as its own sentence, as the old had it in one cue. |
| 40:07 | "시퀀스 투 시퀀스(Sequence to sequence) 또는 Seq2Seq" → "Sequence to Sequence ‖ 또는 2014년에 나온 Sequence to Sequence 모델입니다." | worse | Wrong term correction: the glossary maps "Seek to Seek" to "Sequence to Sequence", so the speaker's two names (sequence to sequence, seq2seq) became the same name twice. The old version had it right. The sentence that follows ends in an unpunctuated stretch on "그러면 이 벡터는", carried on in the next cue. |
| 42:47, 42:53 | Bahtenau → Bahdanau | same | The old Korean already had Bahdanau. |
| 43:59, 44:01 | "그것의 한 단계를 아래에서 천천히 그려보겠습니다. 아래쪽에는…" (run-on) → "그중 한 단계를 천천히 그려 보겠습니다." / "하단에는 Sequence to Sequence의 RNN인 인코더가 있고, ‖ …" | better | The old joined two sentences and attached "at the bottom" to the wrong one. Here "seek to seek" alone is expanded correctly. |
| 44:09 | "…end 토큰까지" → "…N 토큰 뒤의 H8까지 이어집니다." | worse | The new transcript heard "end token" as "N token"; the Korean says "N 토큰". The old had "end 토큰". The segmentation is better, but a term is lost. |
| 44:18 | "plain Seq2Seq는 오직 / 마지막 하나지만 이제 상단에 8개를 모두 유지하는 것은 디코더이며…" → "기존의 Sequence to Sequence는 마지막 하나만 유지했지만, 이제는 여덟 개를 모두 유지합니다." | better | The old cut merged two sentences into a wrong one. |
| 46:19, 46:46, 56:22 | "Marion" → "Marian" | better | Term fixed by the mishearings list. |
| 56:20 | "그 첫 번째 작업은 번역이었습니다." | same | Same sentence. The old cue also held the next sentence about the 2017 Transformer. |
| 58:44 | "BERT의 Both Ways 게임은…" → "BERT의 양방향 방식은 ‖ 11가지 언어 작업에서 새로운 기록을 세웠습니다." | better | The old kept the mishearing "Both Ways game" as a name; the new reads it as "both ways", meaning bidirectional. |
| 60:08, 60:22, 60:25, 60:28, 60:37 | three-line run-on cues ("…cls는 실제 단어가 아니지만, BERT는 모든 문장 앞에 이 토큰을 넣습니다. / 입력값으로 들어가고…") → one sentence or one part per cue | better | The old cut "every / input" across cues. |
| 60:39, 60:40 | "여기 실제 BERT가 있습니다. 그런 방식으로 미세 조정을 하면 / 영화 리뷰에서…" → "여기 진짜 BERT가 있습니다." / "영화 리뷰를 읽으면서 그런 방식으로 미세 조정을 하세요, ‖ 우리 고양이에 관한 두 문장을요." | same | Both misread "fine-tuned that way on movie reviews": the old as a condition ("if you fine-tune"), the new as a command ("do the fine-tuning"). |
| 62:34 | "Burt" → BERT | same | The old Korean already had BERT; the new sentence is in 4 parts. |
| 67:52, 67:59, 68:01 | "the the router" in a cue cut mid-clause ("각 단어는 오직 / 두 명의 전문가에 대해 첫 번째 단어가 들어오면…") → three sentences; "첫 번째 단어인 'the'를 따라가 보세요." | better | The repeat is resolved: the second "the" is the word being followed. |
| 69:54, 70:06, 70:08 | Mixtrel → Mixtral | same | The old Korean already had Mixtral. |
| 0:00 | title sentence | same | Both render the title line. The new keeps "forever" (영원히), which the old dropped, but says "투자하세요" ("invest") for "give me". One gain and one loss, so same. |
| 4:11 | "0보다 크므로 네, 이것이 퍼셉트론 아키텍처의 전부이며 모든 다이어그램은 이 키를 따릅니다." → "이것이 퍼셉트론의 전체 구조입니다." | better | The old cue held three sentences and mistranslated "follows the key along". |
| 22:51 | old cue ending mid-sentence → one sentence | better | The old cue ran into "the 56-layer network can copy the 20-layer one and". The new holds this sentence alone: each layer passes data only to the one above. |
| 42:12 | old cue ending mid-sentence → one sentence | better | The old cut "everything the decoder knows about English / must pass through that one vector" across cues. The new is one sentence in two parts. |
| 60:53 | old cues starting and ending mid-sentence → one sentence | better | The old cut "to answer a question, the small layer scores / every word of the passage twice" across cues. The new is one sentence in two parts. |
| 27:53 | "king 마이너스 / man 더하기 woman" across two cues → "숫자로 표현하면 킹 마이너스 맨 플러스 우먼입니다." | better | The expression is no longer cut in two. |
| 32:15 | "메모리가 u와 TanH를 거쳐 되돌아가기 때문에" → "메모리가 다시 여러분을 거쳐 tanh를 통과하므로" | worse | The new transcript heard the weight "U" as "you", and the Korean says "through you": changed meaning. The sentence is also in an unpunctuated stretch and its last part ends on "단어가 많아지면" ("as the words pile up"). |
| 65:38 | "…프랑스어로 번역하는 것입니다. / 텍스트와 2020년의 GPT-3는…" → "프랑스어로 번역한 다음, 영어 텍스트와 프랑스어 텍스트를 나열하세요." | better | The old cut "the French / text" across cues and joined it to the next sentence. |
| 8:25 | sampled | same | The same sentence ("let's add more neurons"). The old cue also held the next sentence naming the multilayer perceptron. |
| 13:10 | sampled | same | The same sentence ("one perceptron learns only a linear boundary"), without the old "the key idea" opener the new transcript lacks. The hidden-layer sentence the old cue held is its own. |
| 17:58 | sampled | same | The same sentence ("shift the cat 8 pixels right"). The old cue also held the next sentence. |
| 36:57 | sampled | same | The new transcript ends a sentence after "the short-term memory, H." and has "what the output gate reads out of it" as its own (36:59). Together they say what the old cue said. |
| 46:48 | sampled | same | The same sentence ("the grid on the right is one layer of attention"). The old cue also held the next sentence. |
| 51:03 | sampled | same | Identical Korean. |
| 55:23 | sampled | same | The same sentence in two parts (feed-forward boxes are small MLPs; add and norm is a skip connection and normalization). |
| 70:19 | sampled | same | Identical Korean, in two parts. |

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
| 5:08 | 2 sentences in a cue → one sentence in 3 parts | better | The old cue held this sentence and the next (6,000 GPUs for 12 days). The new shows this one alone: these are special computers for heavy workloads like training neural networks. |
| 5:14 | "to get a Lama to 70B" → new transcript "Lama to 7 dB" → Korean "Llama 2 7B" | worse | Wrong term correction: the number became 7B; the old cue said 70B correctly. |
| 6:04, 6:11, 6:22 | rookie numbers; Chat-APT/CLAWD/BARD → ChatGPT/Claude/Bard | same | The old Korean already had the terms right. |
| 12:23 | "음" cue → dropped | better | A filler cue is gone. |
| 13:21, 13:27 | "mostly, mostly" | same | The old Korean had already dropped the repeat. |
| 16:28 | "저희는, 어, 이 과정을…" → "…이 과정을 파인 튜닝이라고 합니다." | better | Filler and false start dropped. |
| 17:32, 17:41 | "사전 학습 단계는, 음, 학습하는 것입니다," / "그것은, 어, 그러니까, 주는 것에 관한, 음, …" → clean sentences | better | The old Korean carried the fillers word for word. |
| 26:58 | "administer a lot of different tests to these large language models" → "예를 들어, 여러분은 다양한" | worse | Lost content: the title hint dropped "tests to these large language models", and the Korean cue breaks off. |
| 33:43 | same sentence | same | Same content in four parts: Greg Brockman showed ChatGPT a pencil sketch of a website. The old cue also held the next sentence. |
| 33:55 | same sentence | same | Same sentence: ChatGPT writes working code for the website from the image. |
| 34:29 | same sentence | same | Same content: not only seeing and making images but audio too. The "multimodality" clause the old cue began with is the sentence before. |
| 34:38 | same sentence | same | Same content: speech-to-speech communication. The old cue also held the next sentence about the iOS app. |
| 54:03 | same sentence | same | Same sentence in two parts: asking Bard to help with the shared Google Doc ("Google Doc" → "Google 문서"). |
| 54:29 | same sentence | same | Same sentence: one way to exfiltrate the data is the following. The old cue also held the next sentence. |
| 56:07 | same sentence | same | Same content: a shared document, a request to summarise, and the data ends up with the attacker. The new is one sentence in three parts, the old four cues. |
| 34:18 | "trashabt" → "챗봇" / new "ChashDBT" → "ChatGPT" | better | Term fixed by the mishearings list. |
| 34:33 | "그래서, 어," → dropped | better | The old Korean kept the filler ("그래서, 어,"). The new drops it: "ChatGPT can now hear and speak". |
| 34:41 | old cue holding two sentences → one sentence | better | The old cue ran into the next sentence about the voice interface. The new is one sentence in three parts: in the iOS app you can talk to ChatGPT as in the film Her. |
| 36:52 | "chong chong …" → "chunk, chunk, …" → "덩어리, 덩어리, 덩어리, 덩어리, 덩어리, 덩어리, 덩어리" | worse | The speaker's sound effect is now the noun "lump" seven times; the old "총총총…" read as a sound. |
| 36:57 | one cue holding two sentences → one each | better | The old cue held two sentences and the loop "총총총…". The new has this sentence alone, "that is how the model samples words in the sequence", without the loop. |
| 36:59 | one cue holding two sentences → one each | better | The second sentence of the old cue on its own: each chunk takes about the same time. |
| 42:39 | "그리고 음" → "그리고," | same | Only a trailing "And um" in both. The new drops "음", but the cue still says nothing on its own. |
| 45:03 | "클래스 시리즈나 Google의 바 시리즈" → "Claude 시리즈나 Google의 Bard 시리즈" | better | Terms fixed. |
| 45:11 | duplicated "빠르게 등장하고 성숙해지는" → one sentence in 4 parts | better | The old Korean repeated a clause. |
| 54:11, 54:15, 54:34 | "BART is hijacked", "marked down" → Bard, 마크다운 형식 | better | Terms fixed; one sentence per cue. |
| 56:27 | "luxe-leap 또는 에이전트 공격" → "백도어 에이전트 공격" | worse | Wrong term correction: the glossary turned the misheard "sleeper agent" into "backdoor agent". The old version was also wrong, but it did not assert a term. |
| 58:40 | "셸 브릭 공격", "백도어" → "탈옥 공격", "백도어 공격" | better | jailbreak was recovered from "shellbreak". |
| 0:00, 3:42, 7:34, 11:11, 14:47, 22:09, 32:39, 36:36, 39:58, 43:40, 50:26 | sampled; the same sentence is split into parts | same | Same meaning; the new parts are in Korean order. |
| 18:55 | "than your higher people" "상위 인력보다…" → "Then you hire people." "그런 다음 사람들을 고용합니다." | better | The new transcript heard the words right. |
| 24:52 | old fragments → one sentence | better | The old split "based on the Mistral series / from another French startup" across cues, running into the next sentence. The new is one sentence. |
| 29:08 | old fragments → one sentence | better | The old gave the answer in four short cues. The new is one sentence in two parts: it collected the information into a table with series A to E. |
| 54:20 | old fragments → one sentence | better | The old split "takes all personal data / and tries to exfiltrate it" across cues with the next sentence. The new is one sentence in three parts. |
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
- Stray one-word cues from windows written before the run-on fix
  (`WJpeW9lCSUM` 1:43, 2:09, 2:58): the video was re-made after the fix.

Fixed in the pipeline after review, for new jobs and for a version whose
grouping is rebuilt. The saved cues keep them until the video is made again:

- A sentence with a comma ended at the run-on limit, which changed or cut its
  meaning: `_MEqhFwnp8o` 24:23, `QWNxQIq0hMo` 2:16, `WjhDDeZ7DvM` 32:26–32:38.
- A period before a lower-case word read as carrying on, so separate
  sentences ran together: `QWNxQIq0hMo` 7:44 ("R1. which").

Remaining, as read above. None was fixed in the saved cues after the final
re-makes; that would need another paid run of the video.

- Lost content:
  - `vSCYL8ySh9g` 12:37 and `zjkBMFhNj_g` 26:58: clauses the title hint dropped;
  - `WJpeW9lCSUM` 10:06 and `_MEqhFwnp8o` 8:41, 20:20: the new transcript differs there;
  - `WjhDDeZ7DvM` 2:44: the example "take you to" lost;
  - `_MEqhFwnp8o` 1:38: a dropped subject.
- Added content:
  - `vSCYL8ySh9g` 14:09: "중요하다고" ("it is important") completes a clause cut by the run-on limit;
  - `WjhDDeZ7DvM` 24:47: a spoken "Um," written as the title phrase;
  - `WJpeW9lCSUM` 20:48: "충분하다는 것입니다" ("is enough") closes a sentence the transcript ended after "3090,", which changes the meaning;
  - `WJpeW9lCSUM` 4:07: the change is stated once per part, with "차이" ("difference") added.
- A filler kept in the Korean: `WJpeW9lCSUM` 12:23 ("Well," → "음,").
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
  - `QDX-1M5Nj7s` 41:44 (a self-correction split);
  - `WJpeW9lCSUM` 2:03 (the head noun moved to the first part, leaving a modifier alone in the second).
  - No content moved across sentences in any place read.
- Wrong term corrections:
  - `WjhDDeZ7DvM` 19:22 (GPT-4Go → GPT-4, should be GPT-4o);
  - `zjkBMFhNj_g` 5:14 (70B → 7B), 56:27 (sleeper → backdoor agent);
  - `_MEqhFwnp8o` 2:48 (Chain of Thought), 17:34 (Nemotron Switchyard), 19:55 and 23:45 (names), 30:00 (COT);
  - `wbc-oxFFV9c` 40:07 (seq2seq → Sequence to Sequence twice), 44:09 (end token → N token);
  - `vSCYL8ySh9g` 14:33 (loop → latent recurrent transformer);
  - `WJpeW9lCSUM` 7:52 (QN 3.6 → Nemotron 3.6, should be Qwen 3.6), 21:02 ("495090" passed through, should be 4090).
- Forced ends in unpunctuated stretches:
  - `QDX-1M5Nj7s` 0:09, 0:23, 0:36 (course name cut);
  - `wbc-oxFFV9c` 4:02, 4:04 (spoken arithmetic cut into fragments);
  - `QWNxQIq0hMo` 7:44, 12:37 (parts ending on "어쩌면", "끝 무렵에는");
  - `WjhDDeZ7DvM` 32:26–32:38 (a trailing fragment).
- Parts too short to read: `vSCYL8ySh9g` 8:35 (two one-word parts).
- A sentence shown twice: `WJpeW9lCSUM` 1:16. Both copies come from one
  writing window. The speech model repeated the window's last sentence in
  280 ms, and the cue builder keeps every segment it is given. It is still
  there after the second re-make.

## Reproducing

The scratch scripts are in `artifacts/sentence-cues/` (ignored by Git, as
they read the person's library):

- `metrics.py` counts the cue shape;
- `pairs.py`, `sel.py` and `compact.py` line up old and new for reading;
- `ko.json` is the measurement's input; the measurement itself is committed
  in `docs/evidence/sentence-cues/measure/`;
- `shots.test.svelte.ts` takes the before/after screenshots;
- `hintdiff.py` compares the transcripts with and without the hint.

The re-make tool is `src-tauri/examples/remake_saved.rs`.
