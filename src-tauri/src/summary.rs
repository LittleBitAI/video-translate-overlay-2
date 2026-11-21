//! The 요약 button: one Gemini call over a ready video's title, source
//! transcript and glossary, held to a response JSON schema. The reply's values
//! are checked here (Gemini enforces the schema's syntax, not its lengths or
//! uniqueness), the checked JSON is stored, and Markdown is rendered from it
//! by a fixed template: no second call, the same Markdown every time.

use crate::api::{deleted_error, err, ApiError, ApiResult};
use crate::gemini::{self, Client, MODEL};
use crate::log;
use serde_json::{json, Value};
use std::sync::Mutex;

/// `level`'s values and how the summary names them.
pub const LEVELS: [(&str, &str); 3] = [("beginner", "입문"), ("intermediate", "중급"), ("advanced", "고급")];

pub(crate) const SYSTEM: &str = "You summarize one technical talk or lecture (deep learning, programming, science) for a Korean \
viewer deciding what it covers and what to study next. The work order is JSON: {\"task\":\"summary\",\"title\",\"terms\",\
\"transcript\"}. transcript is the talk's speech-recognition text in spoken order, and it misspells names and terms; terms \
is the talk's glossary (each term's correct source spelling and its Korean rendering): use it for the correct spelling and \
the Korean of every term. Write gist, every key point and every prerequisite in natural Korean (a prerequisite is a Korean phrase such as 선형대수 기초 or 파이썬 문법, never a bare name), using the glossary's Korean for its \
terms and keeping product, model and code names (such as GPT-4o, PyTorch, ReLU) as spelled. Write topics and next_topics \
in the talk's own language, the language of the transcript (never Korean or any other language for a talk not in it; Latin-letter names are fine), as short phrases a person would search for to find a lecture \
on them. gist is one sentence on what the talk is about. key_points are its main points in the order it makes them. topics \
are the subjects it covers, each named with words the talk itself uses. level is the background it expects. prerequisites are what a viewer should know first, empty \
when it expects nothing. next_topics are subjects to study after it. Say only what the talk says. Reply with JSON only, \
matching the response schema.";

/// The response JSON schema the reply is held to.
pub fn schema() -> Value {
    let list = |description: &str, min: u64| json!({"type": "array", "items": {"type": "string"}, "minItems": min, "description": description});
    json!({
        "type": "object",
        "properties": {
            "gist": {"type": "string", "description": "One sentence in Korean: what the talk is about."},
            "key_points": list("The talk's main points in Korean, in the order it makes them; 3 to 7.", 1),
            "topics": list("The subjects the talk covers, in the talk's own language (not translated), named with words the talk uses; 1 to 8 short phrases.", 1),
            "level": {"type": "string", "enum": LEVELS.map(|(l, _)| l), "description": "The background the talk expects."},
            "prerequisites": list("What to know before the talk, each a phrase in Korean (not a bare name); empty when it expects nothing.", 0),
            "next_topics": list("Subjects to study after the talk, in the talk's own language (not translated); 1 to 5 short search phrases.", 0),
        },
        "required": ["gist", "key_points", "topics", "level", "prerequisites", "next_topics"],
        "additionalProperties": false,
    })
}

pub fn order(title: &str, terms: &Value, transcript: &[String]) -> Value {
    json!({"task": "summary", "title": title, "terms": terms, "transcript": transcript})
}

/// `s` on one line, in Unicode's composed form (NFC), as it is stored.
fn one_line(s: &str) -> String {
    nfc(s).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `s` in Unicode's composed form (NFC): canonically equivalent spellings,
/// such as `é` as one character or as `e` and a combining accent, become one.
fn nfc(s: &str) -> String {
    icu_normalizer::ComposingNormalizerBorrowed::new_nfc().normalize(s).into_owned()
}

fn hangul(s: &str) -> bool {
    s.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c))
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Script { Latin, Hangul, Kana, Han, Cyrillic, Greek, Arabic, Block(u32) }

fn script(c: char) -> Script {
    match c {
        '\u{AC00}'..='\u{D7A3}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' => Script::Hangul,
        '\u{3040}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}' | '\u{FF66}'..='\u{FF9F}' => Script::Kana,
        '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}' | '\u{20000}'..='\u{2FFFF}' => Script::Han,
        '\u{0400}'..='\u{052F}' => Script::Cyrillic,
        '\u{0370}'..='\u{03FF}' => Script::Greek,
        '\u{0600}'..='\u{06FF}' | '\u{0750}'..='\u{077F}' | '\u{08A0}'..='\u{08FF}' | '\u{FB50}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}' => Script::Arabic,
        'A'..='Z' | 'a'..='z' | '\u{00C0}'..='\u{024F}' | '\u{1E00}'..='\u{1EFF}' => Script::Latin,
        // ponytail: other scripts are told apart by 128-code-point block, which the Indic, Thai, Hebrew and
        // most others fill one each; a script spread over blocks counts each block its talk uses.
        c => Script::Block(c as u32 >> 7),
    }
}

/// The scripts a talk writes its own terms in, besides Latin (technical names
/// in any language) and Greek (notation): those of its recognized `language`
/// that its transcript may not show (Han for Korean), and every script that
/// makes up a tenth or more of the letters of its `transcript`, which is in
/// the talk's language whatever code speech recognition gave it (or none).
/// The title and the glossary's Korean targets are not: they can be in
/// another language than the talk.
fn own_scripts(language: Option<&str>, transcript: &str) -> Vec<Script> {
    let mut own: Vec<Script> = match language.map(str::to_ascii_lowercase).as_deref() {
        Some("ko") => vec![Script::Hangul, Script::Han],
        Some("ja") => vec![Script::Kana, Script::Han],
        Some("zh" | "yue") => vec![Script::Han],
        Some("ru" | "uk" | "be" | "bg" | "sr" | "mk" | "kk") => vec![Script::Cyrillic],
        _ => vec![],
    };
    let mut tally: Vec<(Script, usize)> = Vec::new();
    for s in transcript.chars().filter(|c| c.is_alphabetic()).map(script) {
        match tally.iter_mut().find(|(t, _)| *t == s) { Some((_, n)) => *n += 1, None => tally.push((s, 1)) }
    }
    let letters: usize = tally.iter().map(|(_, n)| n).sum();
    for (s, n) in tally {
        if n * 10 >= letters && !matches!(s, Script::Latin | Script::Greek) && !own.contains(&s) { own.push(s); }
    }
    own
}

/// Whether `text` is written in the talk's scripts: every letter is Latin
/// (names), Greek (scientific notation such as β or Δ) or one of `own`.
/// Digits, signs and spaces are not letters.
fn in_scripts(text: &str, own: &[Script]) -> bool {
    text.chars().filter(|c| c.is_alphabetic()).all(|c| { let s = script(c); matches!(s, Script::Latin | Script::Greek) || own.contains(&s) })
}

/// A text's words of three or more letters (two for a Korean word, whose
/// syllables are letters), lowercased, a plural `s` dropped; digits count
/// toward none, so a number alone is no word. A word ends at
/// a character that is not a letter or digit, and where a script written
/// without spaces ([`unspaced`]) meets another: `Transformerの仕組み` holds
/// the word `transformer`.
fn words(text: &str) -> std::collections::HashSet<String> {
    // Letters, not digits, make a word: `2024` is none, `GPT4` is one, `x86` too short.
    runs(text).into_iter().filter(|w| w.chars().filter(|c| c.is_alphabetic()).count() >= if hangul(w) { 2 } else { 3 })
        .map(|w| { let w = w.to_lowercase(); match w.strip_suffix('s') { Some(s) if s.chars().count() >= 3 => s.to_string(), _ => w } })
        .collect()
}

/// A text's runs of letters and digits, as written, split where a script
/// written without spaces ([`unspaced`]) meets another.
fn runs(text: &str) -> Vec<String> {
    let mut runs: Vec<String> = Vec::new();
    let mut last: Option<bool> = None;
    for c in text.chars() {
        let kind = c.is_alphanumeric().then(|| unspaced(script(c)));
        if kind.is_some() && kind != last { runs.push(String::new()); }
        if kind.is_some() { runs.last_mut().unwrap().push(c); }
        last = kind;
    }
    runs
}

/// Whether a run names a term even when too short to be a word: it holds
/// letters and digits (`x86`, `3D`), except a number with a lowercase
/// ending (`2nd`, `100ms`), or two or more letters, all capitals (`AI`). A
/// number (`2024`) or a short lowercase word (`we`, `on`) names none.
fn term(run: &str) -> bool {
    let letters: Vec<char> = run.chars().filter(|c| c.is_alphabetic()).collect();
    let after_number = run.trim_start_matches(|c: char| c.is_numeric());
    match (letters.is_empty(), run.chars().any(char::is_numeric)) {
        (true, _) => false,
        (false, true) => after_number.len() == run.len() || !after_number.chars().all(char::is_lowercase),
        (false, false) => letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase()),
    }
}

/// Whether a letter is in a script written without spaces in a talk with
/// `own` scripts. Han beside Hangul is Hanja, in a script written with spaces.
fn spaceless(c: char, own: &[Script]) -> bool {
    let s = script(c);
    unspaced(s) && !(s == Script::Han && own.contains(&Script::Hangul))
}

/// Whether a talk is written without spaces between words: most letters of
/// its `transcript` are in scripts written without spaces ([`spaceless`]).
/// A Japanese lecture is; an English one quoting a Japanese term is not.
fn written_without_spaces(transcript: &str, own: &[Script]) -> bool {
    let (all, without) = transcript.chars().filter(|c| c.is_alphabetic())
        .fold((0usize, 0usize), |(all, without), c| (all + 1, without + usize::from(spaceless(c, own))));
    without * 2 > all
}

/// Whether a topic is in the talk's own words: one of its words is in the
/// title, glossary or transcript (`source`), or for a Korean word, begins
/// one there (the transcript writes `신경망을` for the topic `신경망`). A
/// same-script translation (French words for an English talk) shares none.
/// A word in a script written without spaces (Han except Hanja, kana, Thai,
/// Lao, Myanmar, Khmer), where nothing shows where words end, must be said
/// as written in a talk written with spaces (`ベイズ` in a talk quoting
/// `ベイズ推定`); a talk written without spaces (`without_spaces`) has no
/// words to compare it with, and it passes. A term too short to be a word
/// ([`term`]: `x86`, `AI`) counts if said whole ([`said_whole`]) in the title,
/// glossary or transcript (`said`), beside other words or alone; so does a
/// name written with signs ([`signed`]: `C++`, `C#`, `Wi-Fi`), and the whole
/// topic (`Go`) if it has a letter. A number never counts.
fn in_talk(topic: &str, own: &[Script], without_spaces: bool, source: &std::collections::HashSet<String>, said: &str) -> bool {
    // A word is all in scripts without spaces or all in others (`words`).
    words(topic).iter().any(|w| match w.chars().all(|c| spaceless(c, own)) {
        true => without_spaces || said.contains(w.as_str()),
        false => source.contains(w) || (hangul(w) && source.iter().any(|s| s.starts_with(w.as_str()))),
    }) || runs(topic).iter().any(|r| term(r) && said_whole(r, said))
        || topic.split_whitespace().filter_map(signed).any(|n| said_whole(n, said))
        || (topic.chars().any(char::is_alphabetic) && said_whole(topic, said))
}

/// A space-free piece of a topic that is a name written with signs, its
/// sentence punctuation at either end left off: it holds a sign other than an
/// apostrophe (`C++`, `C#`, `Wi-Fi`, `U.S`), a letter, and a capital or a
/// digit. `it's`, `e.g` and `well-known` are none: an apostrophe, or no
/// capital or digit, marks an ordinary word.
fn signed(piece: &str) -> Option<&str> {
    let name = piece.trim_matches(|c: char| ",.;:!?\"'()[]{}“”‘’«»".contains(c));
    let sign = name.chars().any(|c| !c.is_alphanumeric());
    let apostrophe = name.contains(['\'', '’']);
    let capital_or_digit = name.chars().any(|c| c.is_uppercase() || c.is_numeric());
    (sign && !apostrophe && capital_or_digit && name.chars().any(char::is_alphabetic)).then_some(name)
}

/// Scripts written without spaces between words: Han, kana, and the blocks
/// of Thai (U+0E00), Lao (U+0E80), Myanmar (U+1000, U+1080) and Khmer
/// (U+1780). Without a dictionary nothing in them shows where a word ends.
fn unspaced(s: Script) -> bool {
    matches!(s, Script::Han | Script::Kana | Script::Block(0x1C | 0x1D | 0x20 | 0x21 | 0x2F))
}

/// Whether `text` holds `topic` as written (case and all), not inside a
/// longer word: a neighbour continues the word only if both it and the
/// topic's edge are letters or digits of scripts written with spaces, and
/// they share a script or one is a digit; a `+` or `#` neighbour continues a
/// name (`C++`, `C#`). `AI` is said in `AI uses` and in `AIの`, not in `MAIN`
/// or `AI2`; `Go` not in `Google`; `x8` not in `x86`; `C` and `C+` not in
/// `C++`; `模型` is said between other Han characters, which mark no word end.
fn said_whole(topic: &str, text: &str) -> bool {
    let continues = |neighbour: Option<char>, edge: Option<char>| match (neighbour, edge) {
        (Some('+' | '#'), Some(_)) => true,
        (Some(n), Some(e)) if n.is_alphanumeric() && e.is_alphanumeric() => {
            let (sn, se) = (script(n), script(e));
            !unspaced(sn) && !unspaced(se) && (sn == se || n.is_numeric() || e.is_numeric())
        }
        _ => false,
    };
    let (first, last) = (topic.chars().next(), topic.chars().next_back());
    text.match_indices(topic).any(|(i, _)| {
        !continues(text[..i].chars().next_back(), first) && !continues(text[i + topic.len()..].chars().next(), last)
    })
}

/// The reply's values as stored, or why they break a value rule. Every
/// prose item (gist, each key point, each prerequisite) holds Korean. Every
/// topic and next topic is written in the talk's scripts ([`own_scripts`],
/// [`in_scripts`]), and every topic, a subject the talk covers, is in the
/// talk's own words ([`in_talk`]); next topics come after the talk and need
/// not be said in it. gist, key_points and topics may not be empty; level is
/// one of [`LEVELS`]. Text is put on one line; blank and repeated list items
/// are dropped. `transcript` is what the talk said; `names` its title and
/// the glossary's source terms, whose words count as the talk's too.
pub fn check(reply: &Value, language: Option<&str>, transcript: &str, names: &str) -> Result<Value, String> {
    // The talk's text and the reply's (through `one_line`) are compared in one Unicode form.
    let (transcript, names) = (&nfc(transcript), nfc(names));
    let own = own_scripts(language, transcript);
    let said = format!("{names}\n{transcript}");
    let source = words(&said);
    let without_spaces = written_without_spaces(transcript, &own);
    let gist = one_line(reply["gist"].as_str().unwrap_or(""));
    if gist.is_empty() { return Err("empty gist".into()); }
    if !hangul(&gist) { return Err("gist is not Korean".into()); }
    let list = |field: &str, min: usize| -> Result<Vec<String>, String> {
        let items = reply[field].as_array().ok_or(format!("{field} is not a list"))?;
        let mut kept: Vec<String> = Vec::new();
        for item in items {
            let text = one_line(item.as_str().ok_or(format!("{field} holds a non-string"))?);
            if !text.is_empty() && !kept.iter().any(|k| k.to_lowercase() == text.to_lowercase()) { kept.push(text); }
        }
        if kept.len() < min { return Err(format!("{field} is empty")); }
        let korean = matches!(field, "key_points" | "prerequisites");
        if korean && !kept.iter().all(|t| hangul(t)) { return Err(format!("{field} is not Korean")); }
        if !korean && !kept.iter().all(|t| in_scripts(t, &own)) {
            return Err(format!("{field} is not in the talk's language"));
        }
        if field == "topics" && !kept.iter().all(|t| in_talk(t, &own, without_spaces, &source, &said)) {
            return Err("a topic is not in the talk's words".into());
        }
        Ok(kept)
    };
    let key_points = list("key_points", 1)?;
    let level = reply["level"].as_str().filter(|l| LEVELS.iter().any(|(k, _)| k == l)).ok_or("unknown level")?;
    Ok(json!({
        "gist": gist,
        "key_points": key_points,
        "topics": list("topics", 1)?,
        "level": level,
        "prerequisites": list("prerequisites", 0)?,
        "next_topics": list("next_topics", 0)?,
    }))
}

/// [`check`] of a reply for the video as the call read it: its transcript
/// is what the talk said, its title and the glossary's source terms are
/// names; the glossary's targets are Korean translations, neither the talk's
/// words nor its scripts.
fn judge(reply: &Value, language: Option<&str>, title: &str, terms: &Value, transcript: &[String]) -> Result<Value, String> {
    let names = std::iter::once(title)
        .chain(terms.as_array().into_iter().flatten().filter_map(|t| t["source"].as_str()))
        .collect::<Vec<_>>().join("\n");
    check(reply, language, &transcript.join("\n"), &names)
}

/// Markdown-escapes one line of model text, so it stays the text of the line it is put on.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\`*_[]<>#|".contains(c) { out.push('\\'); }
        out.push(c);
    }
    // A line that would open a list or a quote.
    let digits = out.chars().take_while(char::is_ascii_digit).count();
    if out.starts_with(['-', '+']) || (digits > 0 && out[digits..].starts_with(['.', ')'])) {
        out.insert(if digits > 0 { digits } else { 0 }, '\\');
    }
    out
}

/// The stored summary as Markdown, by a fixed template.
pub fn markdown(summary: &Value) -> String {
    let items = |field: &str| -> Vec<String> {
        summary[field].as_array().map(|a| a.iter().filter_map(Value::as_str).map(|s| format!("- {}", escape(s))).collect()).unwrap_or_default()
    };
    let list = |field: &str| { let l = items(field); if l.is_empty() { "없음".to_string() } else { l.join("\n") } };
    let level = summary["level"].as_str().and_then(|l| LEVELS.iter().find(|(k, _)| *k == l)).map_or("", |(_, ko)| ko);
    format!(
        "## 한 줄 요약\n\n{}\n\n## 핵심 내용\n\n{}\n\n## 다루는 주제\n\n{}\n\n## 난이도\n\n{level}\n\n## 먼저 알면 좋은 것\n\n{}\n\n## 다음에 볼 주제\n\n{}\n",
        escape(summary["gist"].as_str().unwrap_or("")),
        list("key_points"),
        list("topics"),
        list("prerequisites"),
        list("next_topics"),
    )
}

// ponytail: one summary call at a time in this process; per-video locks if several are ever wanted at once.
static CALLING: Mutex<()> = Mutex::new(());

/// The library file and video whose summary call is out, so a screen opened
/// meanwhile can say a summary is coming and look again.
/// Per library file and video: how many presses are out (each from before
/// it waits its turn behind another video's call, so a queued press is
/// pending too) and how many have started or ended, so a read can tell
/// whether one began or finished while it looked.
static PENDING: Mutex<Vec<(std::path::PathBuf, String, usize, u64)>> = Mutex::new(Vec::new());

struct Pending((std::path::PathBuf, String));

impl Pending {
    fn change(entry: &(std::path::PathBuf, String), out: isize) {
        let mut all = PENDING.lock().unwrap_or_else(|p| p.into_inner());
        match all.iter_mut().find(|(p, k, _, _)| *p == entry.0 && *k == entry.1) {
            Some(e) => { e.2 = e.2.saturating_add_signed(out); e.3 += 1; }
            None => all.push((entry.0.clone(), entry.1.clone(), out.max(0) as usize, 1)),
        }
    }

    fn start(library: &crate::library_scope::Library<'_>, key: &str) -> Result<Pending, ApiError> {
        let entry = (library.run_original(|store| Ok(store.path().to_path_buf()))?, key.to_string());
        Pending::change(&entry, 1);
        Ok(Pending(entry))
    }

    /// Whether a press is out for the video, and how many have started or ended so far.
    fn look(library: &crate::library_scope::Library<'_>, key: &str) -> Result<(bool, u64), ApiError> {
        let path = library.run_original(|store| Ok(store.path().to_path_buf()))?;
        let all = PENDING.lock().unwrap_or_else(|p| p.into_inner());
        Ok(all.iter().find(|(p, k, _, _)| *p == path && k == key).map_or((false, 0), |e| (e.2 > 0, e.3)))
    }
}

impl Drop for Pending {
    fn drop(&mut self) { Pending::change(&self.0, -1); }
}

fn reply(key: &str, stored: &crate::store::Summary, reused: bool) -> ApiResult {
    let body: Value = serde_json::from_str(stored.body.as_deref().unwrap_or("null")).map_err(|e| ApiError::from(format!("stored summary: {e}")))?;
    Ok(json!({"video_key": key, "summary": body, "markdown": markdown(&body), "cost_krw": stored.cost_krw, "reused": reused}))
}

/// `GET` (`get`) and `POST` (`make`, `again`) of `/videos/{video_key}/summary`.
/// `make` answers a stored summary without calling Gemini; `again` always calls.
pub(crate) fn summary_in(library: &crate::library_scope::Library<'_>, key: &str, action: &str) -> ApiResult {
    let core = library.core;
    let row = |library: &crate::library_scope::Library<'_>| library.run(|store| {
        if let Some((token, complete)) = store.deleted(key)? { return Err(deleted_error(token, complete)); }
        if store.video(key)?.is_none() { return Err(err(404, "no_video", "이 영상이 보관함에 없습니다.")); }
        Ok(store.summary(key)?)
    });
    match action {
        "get" => {
            // A call is out for it: the screen shows the loading state and asks again.
            // A call marks itself pending before it opens its token and stops only after
            // its result is written. Looked at before and after the row: if a call was
            // out at either look, or one started or ended between them, the row may not
            // hold what it did, so the answer is pending and the next read is clean.
            let before = Pending::look(library, key)?;
            #[cfg(test)]
            flow_tests::pause("read", library);
            let row = row(library)?;
            let after = Pending::look(library, key)?;
            let pending = before.0 || after.0 || before.1 != after.1;
            let failure = row.as_ref().and_then(|r| r.failure.clone());
            let mut answer = match row.as_ref().filter(|r| r.body.is_some()) {
                Some(s) => reply(key, s, true)?,
                None if pending || failure.is_some() => json!({"video_key": key, "cost_krw": row.as_ref().map_or(0.0, |r| r.cost_krw)}),
                None => return Err(err(404, "no_summary", "이 영상의 요약이 아직 없습니다.")),
            };
            if pending { answer["pending"] = json!(true); }
            // How the last call ended, when it stored nothing: a screen that was waiting for it shows this.
            else if let Some(failure) = failure { answer["failure"] = json!(failure); }
            return Ok(answer);
        }
        "make" | "again" => {}
        _ => return Err(err(400, "bad_summary", "지원하지 않는 요약 요청입니다.")),
    }
    // Pending from before the press waits its turn, through opening the token,
    // until after the result is written (dropped on return).
    let _pending = Pending::start(library, key)?;
    // The entry the press was made on, read before it waits its turn: a call
    // opens its token only on that entry, never on one that replaced it meanwhile.
    let entry = library.run(|store| Ok(store.summary_entry(key)?))?;
    #[cfg(test)]
    flow_tests::pause("queued", library);
    let _calling = CALLING.lock().unwrap_or_else(|p| p.into_inner());
    if action == "make" {
        if let Some(s) = row(library)?.filter(|s| s.body.is_some()) { return reply(key, &s, true); }
    }
    // This call's token on the video's summary row. Deleting the video drops
    // the row, so a reply that returns after a deletion finds no token and is
    // not written, whatever entry is there again under the same ids.
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| ApiError::from(e.to_string()))?;
    let call: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let (title, language, terms, transcript) = library.run(|store| {
        // A video marked for deletion is not summarized: checked before any paid call.
        if let Some((token, complete)) = store.deleted(key)? { return Err(deleted_error(token, complete)); }
        let video = store.video(key)?.ok_or_else(|| err(404, "no_video", "이 영상이 보관함에 없습니다."))?;
        let processing = store.latest_job(key)?.is_some_and(|j| crate::store::ACTIVE.contains(&j.stage.as_str()));
        if processing || store.ready_job(key)?.is_none() {
            return Err(err(409, "not_ready", "자막이 다 만들어진 영상만 요약할 수 있습니다."));
        }
        // The transcript, its glossary and its language all come from one version: a re-make
        // that has not finished rewrote the video row's, and the selected saved version is summarized.
        let (terms, transcript, language) = match store.covered_by_remake(key)? {
            Some(saved) => (json!(saved.terms), saved.cues.into_iter().map(|c| c.source_text).collect::<Vec<_>>(), saved.source_language),
            None => {
                let terms = video.terms.as_deref().and_then(|t| serde_json::from_str(t).ok()).unwrap_or_else(|| json!([]));
                (terms, store.cues(key)?.into_iter().map(|c| c.source_text).collect(), video.source_language)
            }
        };
        let replaced = || err(410, "video_replaced", "요약을 누른 뒤 영상이 삭제되거나 다시 만들어져 요약하지 않았습니다. 다시 눌러 주세요.");
        if !store.open_summary_call(key, &call, entry.as_deref().ok_or_else(replaced)?)? { return Err(replaced()); }
        Ok((video.title, language, terms, transcript))
    })?;
    #[cfg(test)]
    flow_tests::pause("opened", library);
    if transcript.iter().all(|t| t.trim().is_empty()) {
        return Err(err(409, "no_transcript", "요약할 원문이 없습니다."));
    }
    let api_key = core.svc.keystore.get()?.ok_or_else(|| err(428, "no_key", "설정에서 Gemini API 키를 먼저 넣어 주세요."))?;
    let client = Client { base_url: core.cfg.gemini_base_url.clone(), key: api_key, model: MODEL.into() };
    // A call that stores nothing leaves its message on the row, for a screen opened while it was out.
    let failed = |e: ApiError| -> ApiError {
        match library.run_original(|store| Ok(store.fail_summary(key, &call, &e.message)?)) { Ok(()) => e, Err(write) => write }
    };
    let (checked, usage) = match client.order_with(SYSTEM, &order(&title, &terms, &transcript), Some(&schema())) {
        Ok((answer, usage)) => (judge(&answer, language.as_deref(), &title, &terms, &transcript), usage),
        Err(gemini::Error::Reply(e, usage)) => (Err(e), usage),
        Err(gemini::Error::Key(e)) => {
            log(format!("summary key refused: {e}"));
            return Err(failed(err(401, "key_rejected", "Gemini가 개인키를 받아 주지 않았습니다. 설정에서 개인키를 확인해 주세요.")));
        }
        Err(gemini::Error::Transient(e)) => {
            log(format!("summary call failed: {e}"));
            return Err(failed(err(503, "gemini_unreachable", "Gemini에 닿지 않아 요약하지 못했습니다. 잠시 뒤에 다시 시도해 주세요.")));
        }
    };
    // Metered whether or not the reply passed: it was billed either way.
    let body = checked.as_ref().ok().map(Value::to_string);
    let cost = gemini::cost_krw(usage.input, usage.output);
    log(format!("summary {key}: input={} output={} cost_krw={cost:.4} ok={}", usage.input, usage.output, body.is_some()));
    // Into the library the request started in, even if the account changed
    // during the call: the account that paid keeps the result and its usage.
    // The retired screen is then refused by the account check below.
    let invalid = err(502, "summary_invalid", "요약 답이 규칙에 맞지 않아 저장하지 않았습니다. 다시 시도해 주세요.");
    let failure = checked.is_err().then_some(invalid.message.as_str());
    let kept = library.run_original(|store| Ok(store.record_summary(key, &call, usage.input, usage.output, cost, body.as_deref(), failure)?))?;
    library.run(|_| Ok(()))?;
    if !kept {
        return Err(err(410, "video_deleted", "요약하는 동안 영상이 삭제되거나 다시 만들어져 저장하지 않았습니다."));
    }
    if let Err(why) = checked {
        log(format!("summary {key} not stored: {why}"));
        return Err(invalid);
    }
    let s = library.run(|store| Ok(store.summary(key)?))?.ok_or_else(|| ApiError::from("summary row missing".to_string()))?;
    reply(key, &s, false)
}

#[cfg(test)]
#[path = "summary_flow_tests.rs"]
mod flow_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> Value {
        json!({"gist": "어텐션이 무엇인지 설명합니다.", "key_points": ["어텐션은 가중합입니다.", "어텐션은 가중합입니다.", "  "], "topics": ["attention", "Attention", "transformer"],
               "level": "beginner", "prerequisites": [], "next_topics": ["multi-head attention"]})
    }

    #[test]
    fn values_that_break_a_rule_are_refused() {
        assert!(check(&good(), Some("en"), source(Some("en")), "").is_ok());
        for (field, value, why) in [
            ("topics", json!([]), "topics is empty"),
            ("topics", json!(["  "]), "topics is empty"),
            ("level", json!("expert"), "unknown level"),
            ("gist", json!(""), "empty gist"),
            ("gist", json!("About attention."), "gist is not Korean"),
            ("key_points", json!([]), "key_points is empty"),
        ] {
            let mut reply = good();
            reply[field] = value;
            assert_eq!(check(&reply, Some("en"), source(Some("en")), "").unwrap_err(), why, "{field}");
        }
    }

    /// What each table row's talk said: its transcript.
    fn source(language: Option<&str>) -> &'static str {
        match language {
            Some("fr") => "Les réseaux de neurones apprennent des poids.",
            Some("ru") => "Теория вероятностей и статистика.",
            Some("ko") => "선형대수 기초를 먼저 배우고 확률론의 기초로 넘어갑니다. PyTorch 기초도 다룹니다.",
            Some("ja") => "確率論の基礎として、Bayesian inferenceとベイズ推定の考え方を説明します。",
            Some("zh") => "概率论是统计学的基础。",
            Some("ar") => "الشبكات العصبية تتعلم الأوزان من البيانات.",
            Some("hi") => "तंत्रिका नेटवर्क भार सीखते हैं।",
            _ => "Attention is a weighted sum. The transformer uses Bayesian inference; beta decay, or β decay, is physics.",
        }
    }

    /// The property: a reply passes only if every prose item holds Korean,
    /// every topic and next topic is in the talk's scripts (Latin names and
    /// Greek notation allowed in any) whatever code its language was given,
    /// and every topic shares a word with what the talk said. A topic word in
    /// a script written without spaces is held to the talk as written when
    /// the talk is written with spaces, and passes in a talk written without
    /// them; a quoted term in such a script exempts no other word. Expected
    /// results are written out, not computed by the rule.
    #[test]
    fn each_field_is_held_to_its_language() {
        // None is a talk whose language was not recognized; "xx", "ar" and "hi" are codes no table names.
        let latin = "Bayesian inference 2.0";
        let notation = "β decay";
        let plural = "Transformers";
        let unsaid = "quantum chromodynamics";
        let accented = "réseaux de neurones";
        let unaccented = "descente de gradient";
        let hangul = "선형대수 기초";
        let mixed = "PyTorch 기초";
        let kana_han = "確率論の基礎";
        let kana = "ベイズ推定";
        let han = "概率论";
        let cyrillic = "теория вероятностей";
        let hangul_unsaid = "양자 색역학";
        let hangul_stem = "확률론";
        let arabic = "الشبكات العصبية";
        let devanagari = "तंत्रिका नेटवर्क";
        let unsaid_pair = "quantum optics";
        // A talk written with spaces quoting a term in a script without them, and the other way round.
        let quoted = "Attention uses ベイズ推定 and weights.";
        let embedded = "Transformerの仕組みを順番に説明します。";
        let han_run = "今天讲解语言模型训练。";
        let (fr_decomposed, ko_decomposed) = (nfd(source(Some("fr"))), nfd(source(Some("ko"))));
        let accented_decomposed = nfd("réseaux");
        assert!(fr_decomposed.contains("re\u{301}seaux") && ko_decomposed.contains('\u{1112}'), "the decomposed forms differ from the composed ones");
        let by_language = [
            (Some("en"), "key_points", latin, false),
            (Some("en"), "key_points", hangul, true),
            (Some("en"), "prerequisites", latin, false),
            (Some("en"), "prerequisites", hangul, true),
            (Some("en"), "prerequisites", mixed, true),
            (Some("en"), "topics", latin, true),
            (Some("en"), "topics", notation, true),
            (Some("en"), "topics", plural, true),
            (Some("en"), "topics", unsaid, false),
            (Some("en"), "topics", accented, false),
            (Some("en"), "topics", hangul, false),
            (Some("en"), "topics", mixed, false),
            (Some("en"), "topics", kana_han, false),
            (Some("en"), "topics", han, false),
            (Some("en"), "next_topics", kana, false),
            (Some("en"), "next_topics", cyrillic, false),
            (Some("en"), "next_topics", notation, true),
            // Next topics are held to the talk's scripts only (the person's choice): a
            // translation in the same alphabet is not caught, with or without accents.
            (Some("en"), "next_topics", unsaid, true),
            (Some("en"), "next_topics", accented, true),
            (Some("en"), "next_topics", unaccented, true),
            (Some("fr"), "topics", accented, true),
            (Some("fr"), "topics", latin, false),
            (Some("fr"), "next_topics", kana, false),
            (Some("ja"), "topics", kana_han, true),
            (Some("ja"), "next_topics", kana, true),
            (Some("ja"), "topics", latin, true),
            (Some("ja"), "topics", hangul, false),
            (Some("zh"), "topics", han, true),
            (Some("zh"), "topics", kana, false),
            (Some("ko"), "topics", hangul, true),
            (Some("ko"), "topics", mixed, true),
            (Some("ko"), "topics", hangul_stem, true),
            (Some("ko"), "topics", hangul_unsaid, false),
            (Some("ko"), "next_topics", hangul_unsaid, true),
            (Some("ko"), "next_topics", mixed, true),
            (Some("ko"), "topics", kana, false),
            (Some("ru"), "topics", cyrillic, true),
            (Some("ru"), "topics", han, false),
            // A code no table names: the talk's own text says which scripts are its own.
            (Some("ar"), "topics", arabic, true),
            (Some("ar"), "topics", kana_han, false),
            (Some("ar"), "next_topics", kana, false),
            (Some("ar"), "next_topics", devanagari, false),
            (Some("ar"), "next_topics", latin, true),
            (Some("hi"), "topics", devanagari, true),
            (Some("hi"), "next_topics", arabic, false),
            (Some("xx"), "topics", kana_han, false),
            (Some("xx"), "topics", latin, true),
            (None, "topics", hangul, false),
            (None, "next_topics", kana, false),
            (None, "topics", latin, true),
        ]
        .map(|(language, field, item, kept)| (language, source(language), field, item, kept));
        let mixed = [
            // Each topic word is compared by its own script: a quoted term does not exempt the talk's Latin topics.
            (Some("en"), quoted, "topics", unsaid_pair, false),
            (Some("en"), quoted, "topics", "weights", true),
            // In a talk written with spaces, a word in a script without them must be said as written.
            (Some("en"), quoted, "topics", "ベイズ", true),
            (Some("en"), quoted, "topics", "カタカナ", false),
            // A talk written without spaces has no words to hold such a topic to.
            (Some("ja"), source(Some("ja")), "topics", "カタカナ", true),
            // Its two Han letters are under a tenth of the talk's letters, so Han is not the talk's script.
            (Some("en"), quoted, "topics", "ベイズ推定", false),
            (Some("ja"), embedded, "topics", "Transformer", true),
            (Some("ja"), embedded, "topics", unsaid_pair, false),
            (Some("zh"), han_run, "topics", "深度学习方法", true),
            // Canonically equivalent spellings are one word, the talk's or the topic's decomposed.
            (Some("fr"), fr_decomposed.as_str(), "topics", "réseaux", true),
            (Some("fr"), source(Some("fr")), "topics", accented_decomposed.as_str(), true),
            (Some("fr"), fr_decomposed.as_str(), "topics", "poissons rouges", false),
            (Some("ko"), ko_decomposed.as_str(), "topics", hangul_stem, true),
            (Some("ko"), ko_decomposed.as_str(), "topics", hangul_unsaid, false),
        ];
        let mut wrong = Vec::new();
        for (language, transcript, field, item, kept) in by_language.into_iter().chain(mixed) {
            // The rest of the reply passes for this talk (its topic is the talk's first word), so only `item` decides.
            let mut reply = good();
            reply["topics"] = json!([transcript.split_whitespace().next().unwrap()]);
            assert!(check(&reply, language, transcript, "").is_ok(), "{language:?} {transcript:?}: the base reply");
            reply[field] = json!([item]);
            if check(&reply, language, transcript, "").is_ok() != kept { wrong.push(format!("{language:?} {transcript:?} {field} {item:?} kept={kept}")); }
        }
        assert!(wrong.is_empty(), "rows with the wrong result:\n{}", wrong.join("\n"));
    }

    #[test]
    fn checked_values_drop_blanks_and_repeats() {
        let c = check(&good(), Some("en"), source(Some("en")), "").unwrap();
        assert_eq!(c["key_points"], json!(["어텐션은 가중합입니다."]));
        assert_eq!(c["topics"], json!(["attention", "transformer"]));
        // Two spellings of one word are one item, stored composed.
        let mut reply = good();
        reply["topics"] = json!([nfd("réseaux"), "réseaux"]);
        let c = check(&reply, Some("fr"), source(Some("fr")), "").unwrap();
        assert_eq!(c["topics"], json!(["réseaux"]));
    }

    /// `s` decomposed (NFD): `é` as `e` and a combining accent, a Hangul syllable as its letters.
    fn nfd(s: &str) -> String {
        icu_normalizer::DecomposingNormalizerBorrowed::new_nfd().normalize(s).into_owned()
    }

    /// The property: a topic too short to share a word with the talk passes
    /// only if the talk says it whole, as written, not inside a longer run of
    /// the same script or beside a digit of one, in a talk written with
    /// spaces or without; a topic with no letter never passes.
    #[test]
    fn a_short_topic_must_be_said_whole() {
        let spaced = "Modern AI uses attention.";
        let inside = "The MAIN loop and Google use attention.";
        let named = "C++ and attention.";
        let unsaid = "Attention uses weights.";
        let spaceless = "確率論の基礎とAIの話です。";
        let korean = "AI 기초를 배웁니다.";
        let han_run = "今天讲解语言模型训练。";
        for (name, language, transcript, topic, kept) in [
            ("said in a spaced talk", "en", spaced, "AI", true),
            ("never said", "en", spaced, "Go", false),
            ("said in another case", "en", spaced, "ai", false),
            ("only inside a longer word", "en", inside, "AI", false),
            ("only at the start of a longer word", "en", inside, "Go", false),
            ("only before a digit of a longer name", "en", "The AI2 model uses attention.", "AI", false),
            ("said with its signs", "en", named, "C++", true),
            ("with signs, never said", "en", unsaid, "C++", false),
            ("no letter or digit", "en", named, "++", false),
            ("said in a talk without spaces", "ja", spaceless, "AI", true),
            ("never said in a talk without spaces", "ja", spaceless, "Go", false),
            // In a script written without spaces a same-script neighbour says nothing about where a word ends.
            ("said between Han characters", "zh", han_run, "模型", true),
            ("never said between Han characters", "zh", han_run, "数据", false),
            ("said in its own script in a talk without spaces", "ja", spaceless, "確率", true),
            ("never said in its own script in a talk without spaces", "ja", spaceless, "統計", false),
            ("said before a Korean word", "ko", korean, "AI", true),
        ] {
            // The base reply's topic is the talk's first word, so only `topic` decides.
            let mut reply = good();
            reply["topics"] = json!([transcript.split_whitespace().next().unwrap()]);
            assert!(check(&reply, Some(language), transcript, "").is_ok(), "{name}: the base reply");
            reply["topics"] = json!([topic]);
            assert_eq!(check(&reply, Some(language), transcript, "").is_ok(), kept, "{name}");
        }
    }

    /// The property: a number never makes a topic the talk's, alone or
    /// beside words the talk never says. A term too short to be a word that
    /// mixes letters and digits or is all capitals counts when the talk says
    /// it whole, alone or beside unsaid words; a number with a lowercase
    /// ending, a short lowercase word and a term said only inside a longer
    /// one do not.
    #[test]
    fn a_number_never_counts_and_a_short_term_counts_said_whole() {
        let talk = "In 2024 we studied attention with GPT4 on x86 code, a 3D model for AI on the 2nd day.";
        for (name, topic, kept) in [
            ("a number the talk says, alone", "2024", false),
            ("a number the talk says, beside unsaid words", "2024 alpha beta", false),
            ("a number never said", "1999", false),
            ("a number beside a word the talk says", "attention 2024", true),
            ("a word of letters and digits the talk says", "GPT4", true),
            ("a short letters-and-digits term, alone", "x86", true),
            ("a short letters-and-digits term, beside an unsaid word", "x86 alpha", true),
            ("a short term beginning with a number and a capital, beside an unsaid word", "3D alpha", true),
            ("a capitals term, beside an unsaid word", "AI alpha", true),
            ("a short letters-and-digits term never said", "x64 alpha", false),
            ("a short term said only inside a longer one", "x8 alpha", false),
            ("a number with a lowercase ending", "2nd alpha", false),
            ("a short lowercase word the talk says", "we alpha", false),
            ("a capitals term the talk says in another case", "ON alpha", false),
        ] {
            let mut reply = good();
            reply["topics"] = json!([topic]);
            assert_eq!(check(&reply, Some("en"), talk, "").is_ok(), kept, "{name}");
        }
    }

    /// The property: a name written with signs (a sign other than an
    /// apostrophe, a letter, a capital or digit) counts when the talk says it
    /// whole, beside words the talk never says or alone; sentence punctuation
    /// at its ends is not part of it. A name the talk never says, says only
    /// inside a longer name or in another case, and an ordinary word with
    /// signs (an apostrophe, or no capital or digit) do not count.
    #[test]
    fn a_name_with_signs_counts_said_whole_inside_a_phrase() {
        let talk = "With C++ templates and C# delegates, Wi-Fi links reach the U.S. grid; it's slow, e.g. at night.";
        for (name, topic, kept) in [
            ("a name ending in signs, beside an unsaid word", "C++ alpha", true),
            ("a name ending in a sign, beside an unsaid word", "C# alpha", true),
            ("a hyphenated name with capitals, beside an unsaid word", "Wi-Fi alpha", true),
            ("a dotted name, with its closing dot, beside an unsaid word", "U.S. alpha", true),
            ("a name ending in signs, followed by a comma in the topic", "C++, alpha", true),
            ("a name ending in signs, alone", "C++", true),
            ("a name with signs never said", "F# alpha", false),
            ("a name said only inside a longer one", "C+ alpha", false),
            ("a letter said only before signs", "C alpha", false),
            ("a name said in another case", "c++ alpha", false),
            ("a word with an apostrophe", "it's alpha", false),
            ("a dotted lowercase abbreviation", "e.g. alpha", false),
        ] {
            let mut reply = good();
            reply["topics"] = json!([topic]);
            assert_eq!(check(&reply, Some("en"), talk, "").is_ok(), kept, "{name}");
        }
    }

    /// The property: only what the talk said decides its scripts. A title or
    /// a glossary target in another script lends the talk neither that script
    /// nor its words; the title's words and the glossary's source terms count
    /// as the talk's words. Run through `judge`, which the call uses.
    #[test]
    fn only_the_transcript_decides_the_talk_s_scripts() {
        let said = vec!["Attention uses weights.".to_string()];
        let plain_title = "Attention";
        let other_title = "어텐션 가중합 계산 방식 강의";
        let translated = json!([{"source": "attention", "target": "어텐션 가중합 계산 방식"}]);
        let unsaid_term = json!([{"source": "softmax", "target": "소프트맥스"}]);
        let none = json!([]);
        let korean = "확률론";
        for (name, title, terms, field, item, kept) in [
            ("a translated glossary, a Korean next topic", plain_title, &translated, "next_topics", korean, false),
            ("a translated glossary, its target as a topic", plain_title, &translated, "topics", "어텐션", false),
            ("a title in another script, a Korean next topic", other_title, &none, "next_topics", korean, false),
            ("a title in another script, a Korean topic from it", other_title, &none, "topics", "어텐션", false),
            ("no glossary, a Korean next topic", plain_title, &none, "next_topics", korean, false),
            ("a glossary source term the transcript lacks, as a topic", plain_title, &unsaid_term, "topics", "softmax", true),
            ("a glossary target's word as a topic", plain_title, &unsaid_term, "topics", "소프트맥스", false),
            ("the title's word as a topic", "Attention lecture", &none, "topics", "lecture", true),
        ] {
            let mut reply = good();
            reply["topics"] = json!(["attention"]);
            assert!(judge(&reply, Some("en"), title, terms, &said).is_ok(), "{name}: the base reply");
            reply[field] = json!([item]);
            assert_eq!(judge(&reply, Some("en"), title, terms, &said).is_ok(), kept, "{name}");
        }
    }

    #[test]
    fn the_same_json_always_renders_the_same_markdown() {
        // `markdown` takes only the stored JSON: there is no client to call a model with.
        let stored = check(&json!({"gist": "C#과 *포인터*를 다룹니다.", "key_points": ["- 첫째", "1. 둘째"], "topics": ["__init__", "C++"],
                                   "level": "advanced", "prerequisites": ["파이썬"], "next_topics": []}), Some("en"), "__init__ C++", "").unwrap();
        let once = markdown(&stored);
        assert_eq!(once, markdown(&serde_json::from_str(&stored.to_string()).unwrap()));
        assert_eq!(
            once,
            "## 한 줄 요약\n\nC\\#과 \\*포인터\\*를 다룹니다.\n\n## 핵심 내용\n\n- \\- 첫째\n- 1\\. 둘째\n\n## 다루는 주제\n\n- \\_\\_init\\_\\_\n- C++\n\n\
             ## 난이도\n\n고급\n\n## 먼저 알면 좋은 것\n\n- 파이썬\n\n## 다음에 볼 주제\n\n없음\n"
        );
    }
}
