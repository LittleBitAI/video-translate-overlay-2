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

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn hangul(s: &str) -> bool {
    s.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c))
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Script { Latin, Hangul, Kana, Han, Cyrillic, Greek, Other }

fn script(c: char) -> Script {
    match c {
        '\u{AC00}'..='\u{D7A3}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' => Script::Hangul,
        '\u{3040}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}' | '\u{FF66}'..='\u{FF9F}' => Script::Kana,
        '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}' | '\u{20000}'..='\u{2FFFF}' => Script::Han,
        '\u{0400}'..='\u{052F}' => Script::Cyrillic,
        '\u{0370}'..='\u{03FF}' => Script::Greek,
        'A'..='Z' | 'a'..='z' | '\u{00C0}'..='\u{024F}' | '\u{1E00}'..='\u{1EFF}' => Script::Latin,
        _ => Script::Other,
    }
}

/// The scripts a talk in `language` (a speech-recognition language code)
/// writes its own terms in, besides Latin, which technical names use in any
/// language. `None` for a language this table does not know.
fn scripts(language: &str) -> Option<&'static [Script]> {
    const LATIN: [&str; 33] = ["en", "fr", "de", "es", "it", "pt", "nl", "sv", "da", "no", "nb", "nn", "fi", "pl", "cs", "sk", "sl",
        "hr", "ro", "hu", "tr", "id", "ms", "vi", "ca", "et", "lv", "lt", "af", "sw", "tl", "eu", "gl"];
    Some(match language.to_ascii_lowercase().as_str() {
        "ko" => &[Script::Hangul, Script::Han],
        "ja" => &[Script::Kana, Script::Han],
        "zh" | "yue" => &[Script::Han],
        "ru" | "uk" | "be" | "bg" | "sr" | "mk" | "kk" => &[Script::Cyrillic],
        "el" => &[Script::Greek],
        l if LATIN.contains(&l) => &[],
        _ => return None,
    })
}

/// Whether `text` is written in `language`'s scripts: every letter is Latin
/// (names), Greek (scientific notation such as β or Δ) or in one of the
/// language's own scripts. Digits, signs and spaces are not letters.
fn in_scripts(text: &str, language: &str) -> bool {
    let Some(own) = scripts(language) else { return true };
    text.chars().filter(|c| c.is_alphabetic()).all(|c| { let s = script(c); matches!(s, Script::Latin | Script::Greek) || own.contains(&s) })
}

/// A text's words of three or more letters, lowercased, a plural `s` dropped.
fn words(text: &str) -> std::collections::HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 3)
        .map(|w| { let w = w.to_lowercase(); match w.strip_suffix('s') { Some(s) if s.chars().count() >= 3 => s.to_string(), _ => w } })
        .collect()
}

/// Whether a topic is in the talk's own words, for a language written with
/// spaces between words (Latin, Cyrillic and Greek scripts): one of its words
/// is in the title, glossary or transcript (`source`). A same-script
/// translation (French words for an English talk) shares none. A topic with
/// no word of three letters (`C++`, `AI`) has nothing to judge.
fn in_talk(topic: &str, language: &str, source: &std::collections::HashSet<String>) -> bool {
    if matches!(language.to_ascii_lowercase().as_str(), "ko" | "ja" | "zh" | "yue") || scripts(language).is_none() { return true; }
    let own = words(topic);
    own.is_empty() || own.iter().any(|w| source.contains(w))
}

/// The reply's values as stored, or why they break a value rule. Every
/// prose item (gist, each key point, each prerequisite) holds Korean. For a
/// talk whose recognized `language` is known, every topic and next topic is
/// written in its scripts ([`in_scripts`]), and every topic, a subject the
/// talk covers, is in the talk's own words ([`in_talk`]); next topics come
/// after the talk and need not be said in it. gist, key_points and topics may
/// not be empty; level is one of [`LEVELS`]. Text is put on one line; blank
/// and repeated list items are dropped. `source` is the talk's title, glossary
/// and transcript.
pub fn check(reply: &Value, language: Option<&str>, source: &str) -> Result<Value, String> {
    let source = words(source);
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
        if !korean && language.is_some_and(|l| !kept.iter().all(|t| in_scripts(t, l))) {
            return Err(format!("{field} is not in the talk's language"));
        }
        if field == "topics" && language.is_some_and(|l| !kept.iter().all(|t| in_talk(t, l, &source))) {
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
static PENDING: Mutex<Option<(std::path::PathBuf, String)>> = Mutex::new(None);

struct Pending;

impl Pending {
    fn start(library: &crate::library_scope::Library<'_>, key: &str) -> Result<Pending, ApiError> {
        let path = library.run_original(|store| Ok(store.path().to_path_buf()))?;
        *PENDING.lock().unwrap_or_else(|p| p.into_inner()) = Some((path, key.to_string()));
        Ok(Pending)
    }

    fn is(library: &crate::library_scope::Library<'_>, key: &str) -> Result<bool, ApiError> {
        let path = library.run_original(|store| Ok(store.path().to_path_buf()))?;
        Ok(PENDING.lock().unwrap_or_else(|p| p.into_inner()).as_ref().is_some_and(|(p, k)| *p == path && k == key))
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        *PENDING.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }
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
            // Looked at before the row: a call marks itself pending before it opens
            // its token and stops being pending only after its result is written, so
            // "not pending" here means the row read next holds every finished call.
            let pending = Pending::is(library, key)?;
            #[cfg(test)]
            flow_tests::pause("read", library);
            let row = row(library)?;
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
    let _calling = CALLING.lock().unwrap_or_else(|p| p.into_inner());
    if action == "make" {
        if let Some(s) = row(library)?.filter(|s| s.body.is_some()) { return reply(key, &s, true); }
    }
    // This call's token on the video's summary row. Deleting the video drops
    // the row, so a reply that returns after a deletion finds no token and is
    // not written, whatever entry is there again under the same ids.
    // Pending from before the token is opened until after the result is written (dropped on return).
    let _pending = Pending::start(library, key)?;
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
        let (terms, transcript) = match store.covered_by_remake(key)? {
            Some(saved) => (json!(saved.terms), saved.cues.into_iter().map(|c| c.source_text).collect::<Vec<_>>()),
            None => {
                let terms = video.terms.as_deref().and_then(|t| serde_json::from_str(t).ok()).unwrap_or_else(|| json!([]));
                (terms, store.cues(key)?.into_iter().map(|c| c.source_text).collect())
            }
        };
        store.open_summary_call(key, &call)?;
        Ok((video.title, video.source_language, terms, transcript))
    })?;
    #[cfg(test)]
    flow_tests::pause("opened", library);
    if transcript.iter().all(|t| t.trim().is_empty()) {
        return Err(err(409, "no_transcript", "요약할 원문이 없습니다."));
    }
    let api_key = core.svc.keystore.get()?.ok_or_else(|| err(428, "no_key", "설정에서 Gemini API 키를 먼저 넣어 주세요."))?;
    let client = Client { base_url: core.cfg.gemini_base_url.clone(), key: api_key, model: MODEL.into() };
    let source = format!("{title}\n{terms}\n{}", transcript.join("\n"));
    // A call that stores nothing leaves its message on the row, for a screen opened while it was out.
    let failed = |e: ApiError| -> ApiError {
        match library.run_original(|store| Ok(store.fail_summary(key, &call, &e.message)?)) { Ok(()) => e, Err(write) => write }
    };
    let (checked, usage) = match client.order_with(SYSTEM, &order(&title, &terms, &transcript), Some(&schema())) {
        Ok((answer, usage)) => (check(&answer, language.as_deref(), &source), usage),
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
        assert!(check(&good(), Some("en"), source(Some("en"))).is_ok());
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
            assert_eq!(check(&reply, Some("en"), source(Some("en"))).unwrap_err(), why, "{field}");
        }
    }

    /// What each table row's talk said: its title, glossary and transcript.
    fn source(language: Option<&str>) -> &'static str {
        match language {
            Some("fr") => "Les réseaux de neurones apprennent des poids.",
            Some("ru") => "Теория вероятностей и статистика.",
            _ => "Attention is a weighted sum. The transformer uses Bayesian inference; beta decay, or β decay, is physics.",
        }
    }

    /// The property: a reply passes only if every prose item holds Korean
    /// and, for a talk whose language is known, every topic and next topic is
    /// in that language's scripts (Latin names and Greek notation allowed in
    /// any), and every topic shares a word with what a talk written with
    /// spaces said. Expected results are written out, not computed by the rule.
    #[test]
    fn each_field_is_held_to_its_language() {
        // None is a talk whose language was not recognized; "xx" a code the table does not know.
        let latin = "Bayesian inference 2.0";
        let notation = "β decay";
        let plural = "Transformers";
        let unsaid = "quantum chromodynamics";
        let no_word = "C++";
        let accented = "réseaux de neurones";
        let unaccented = "descente de gradient";
        let hangul = "선형대수 기초";
        let mixed = "PyTorch 기초";
        let kana_han = "確率論の基礎";
        let kana = "ベイズ推定";
        let han = "概率论";
        let cyrillic = "теория вероятностей";
        for (language, field, item, kept) in [
            (Some("en"), "key_points", latin, false),
            (Some("en"), "key_points", hangul, true),
            (Some("en"), "prerequisites", latin, false),
            (Some("en"), "prerequisites", hangul, true),
            (Some("en"), "prerequisites", mixed, true),
            (Some("en"), "topics", latin, true),
            (Some("en"), "topics", notation, true),
            (Some("en"), "topics", plural, true),
            (Some("en"), "topics", no_word, true),
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
            (Some("ko"), "next_topics", mixed, true),
            (Some("ko"), "topics", kana, false),
            (Some("ru"), "topics", cyrillic, true),
            (Some("ru"), "topics", han, false),
            (Some("xx"), "topics", kana_han, true),
            (None, "topics", hangul, true),
            (None, "next_topics", kana, true),
        ] {
            let mut reply = good();
            reply[field] = json!([item]);
            assert_eq!(check(&reply, language, source(language)).is_ok(), kept, "{language:?} {field} {item:?}");
        }
    }

    #[test]
    fn checked_values_drop_blanks_and_repeats() {
        let c = check(&good(), Some("en"), source(Some("en"))).unwrap();
        assert_eq!(c["key_points"], json!(["어텐션은 가중합입니다."]));
        assert_eq!(c["topics"], json!(["attention", "transformer"]));
    }

    #[test]
    fn the_same_json_always_renders_the_same_markdown() {
        // `markdown` takes only the stored JSON: there is no client to call a model with.
        let stored = check(&json!({"gist": "C#과 *포인터*를 다룹니다.", "key_points": ["- 첫째", "1. 둘째"], "topics": ["__init__", "C++"],
                                   "level": "advanced", "prerequisites": ["파이썬"], "next_topics": []}), Some("en"), "__init__ C++").unwrap();
        let once = markdown(&stored);
        assert_eq!(once, markdown(&serde_json::from_str(&stored.to_string()).unwrap()));
        assert_eq!(
            once,
            "## 한 줄 요약\n\nC\\#과 \\*포인터\\*를 다룹니다.\n\n## 핵심 내용\n\n- \\- 첫째\n- 1\\. 둘째\n\n## 다루는 주제\n\n- \\_\\_init\\_\\_\n- C++\n\n\
             ## 난이도\n\n고급\n\n## 먼저 알면 좋은 것\n\n- 파이썬\n\n## 다음에 볼 주제\n\n없음\n"
        );
    }
}
