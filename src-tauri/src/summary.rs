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
the Korean of every term. Write gist, key_points and prerequisites in natural Korean, using the glossary's Korean for its \
terms and keeping product, model and code names (such as GPT-4o, PyTorch, ReLU) as spelled. Write topics and next_topics \
in the talk's own language, the language of the transcript, as short phrases a person would search for to find a lecture \
on them. gist is one sentence on what the talk is about. key_points are its main points in the order it makes them. topics \
are the subjects it covers. level is the background it expects. prerequisites are what a viewer should know first, empty \
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
            "topics": list("The subjects the talk covers, in the talk's own language (not translated); 1 to 8 short phrases.", 1),
            "level": {"type": "string", "enum": LEVELS.map(|(l, _)| l), "description": "The background the talk expects."},
            "prerequisites": list("What to know before the talk, in Korean; empty when it expects nothing.", 0),
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

/// The reply's values as stored, or why they break a value rule: an empty
/// or non-Korean gist or key point, no topics or key points, an unknown level.
/// Text is put on one line; blank and repeated list items are dropped.
pub fn check(reply: &Value) -> Result<Value, String> {
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
        Ok(kept)
    };
    let key_points = list("key_points", 1)?;
    if !key_points.iter().all(|p| hangul(p)) { return Err("a key point is not Korean".into()); }
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

fn reply(key: &str, stored: &crate::store::Summary, reused: bool) -> ApiResult {
    let body: Value = serde_json::from_str(stored.body.as_deref().unwrap_or("null")).map_err(|e| ApiError::from(format!("stored summary: {e}")))?;
    Ok(json!({"video_key": key, "summary": body, "markdown": markdown(&body), "cost_krw": stored.cost_krw, "reused": reused}))
}

/// `GET` (`get`) and `POST` (`make`, `again`) of `/videos/{video_key}/summary`.
/// `make` answers a stored summary without calling Gemini; `again` always calls.
pub(crate) fn summary_in(library: &crate::library_scope::Library<'_>, key: &str, action: &str) -> ApiResult {
    let core = library.core;
    let stored = |library: &crate::library_scope::Library<'_>| library.run(|store| {
        if let Some((token, complete)) = store.deleted(key)? { return Err(deleted_error(token, complete)); }
        if store.video(key)?.is_none() { return Err(err(404, "no_video", "이 영상이 보관함에 없습니다.")); }
        Ok(store.summary(key)?.filter(|s| s.body.is_some()))
    });
    match action {
        "get" => return stored(library)?.map_or_else(|| Err(err(404, "no_summary", "이 영상의 요약이 아직 없습니다.")), |s| reply(key, &s, true)),
        "make" | "again" => {}
        _ => return Err(err(400, "bad_summary", "지원하지 않는 요약 요청입니다.")),
    }
    let _calling = CALLING.lock().unwrap_or_else(|p| p.into_inner());
    if action == "make" {
        if let Some(s) = stored(library)? { return reply(key, &s, true); }
    }
    let (title, terms, transcript) = library.run(|store| {
        let video = store.video(key)?.ok_or_else(|| err(404, "no_video", "이 영상이 보관함에 없습니다."))?;
        let processing = store.latest_job(key)?.is_some_and(|j| crate::store::ACTIVE.contains(&j.stage.as_str()));
        if processing || store.ready_job(key)?.is_none() {
            return Err(err(409, "not_ready", "자막이 다 만들어진 영상만 요약할 수 있습니다."));
        }
        Ok(match store.covered_by_remake(key)? {
            Some(saved) => (video.title, json!(saved.terms), saved.cues.into_iter().map(|c| c.source_text).collect::<Vec<_>>()),
            None => {
                let terms = video.terms.as_deref().and_then(|t| serde_json::from_str(t).ok()).unwrap_or_else(|| json!([]));
                (video.title, terms, store.cues(key)?.into_iter().map(|c| c.source_text).collect())
            }
        })
    })?;
    if transcript.iter().all(|t| t.trim().is_empty()) {
        return Err(err(409, "no_transcript", "요약할 원문이 없습니다."));
    }
    let api_key = core.svc.keystore.get()?.ok_or_else(|| err(428, "no_key", "설정에서 Gemini API 키를 먼저 넣어 주세요."))?;
    let client = Client { base_url: core.cfg.gemini_base_url.clone(), key: api_key, model: MODEL.into() };
    let (checked, usage) = match client.order_with(SYSTEM, &order(&title, &terms, &transcript), Some(&schema())) {
        Ok((answer, usage)) => (check(&answer), usage),
        Err(gemini::Error::Reply(e, usage)) => (Err(e), usage),
        Err(gemini::Error::Key(e)) => {
            log(format!("summary key refused: {e}"));
            return Err(err(401, "key_rejected", "Gemini가 개인키를 받아 주지 않았습니다. 설정에서 개인키를 확인해 주세요."));
        }
        Err(gemini::Error::Transient(e)) => {
            log(format!("summary call failed: {e}"));
            return Err(err(503, "gemini_unreachable", "Gemini에 닿지 않아 요약하지 못했습니다. 잠시 뒤에 다시 시도해 주세요."));
        }
    };
    // Metered whether or not the reply passed: it was billed either way.
    let body = checked.as_ref().ok().map(Value::to_string);
    let cost = gemini::cost_krw(usage.input, usage.output);
    log(format!("summary {key}: input={} output={} cost_krw={cost:.4} ok={}", usage.input, usage.output, body.is_some()));
    let kept = library.run(|store| Ok(store.record_summary(key, usage.input, usage.output, cost, body.as_deref())?))?;
    if !kept {
        return Err(err(410, "video_deleted", "요약하는 동안 영상이 삭제됐습니다."));
    }
    if let Err(why) = checked {
        log(format!("summary {key} not stored: {why}"));
        return Err(err(502, "summary_invalid", "요약 답이 규칙에 맞지 않아 저장하지 않았습니다. 다시 시도해 주세요."));
    }
    let s = library.run(|store| Ok(store.summary(key)?))?.ok_or_else(|| ApiError::from("summary row missing".to_string()))?;
    reply(key, &s, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> Value {
        json!({"gist": "어텐션이 무엇인지 설명합니다.", "key_points": ["어텐션은 가중합입니다.", "어텐션은 가중합입니다.", "  "], "topics": ["attention", "Attention", "transformer"],
               "level": "beginner", "prerequisites": [], "next_topics": ["multi-head attention"]})
    }

    #[test]
    fn values_that_break_a_rule_are_refused() {
        assert!(check(&good()).is_ok());
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
            assert_eq!(check(&reply).unwrap_err(), why, "{field}");
        }
    }

    #[test]
    fn checked_values_drop_blanks_and_repeats() {
        let c = check(&good()).unwrap();
        assert_eq!(c["key_points"], json!(["어텐션은 가중합입니다."]));
        assert_eq!(c["topics"], json!(["attention", "transformer"]));
    }

    #[test]
    fn the_same_json_always_renders_the_same_markdown() {
        // `markdown` takes only the stored JSON: there is no client to call a model with.
        let stored = check(&json!({"gist": "C#과 *포인터*를 다룹니다.", "key_points": ["- 첫째", "1. 둘째"], "topics": ["__init__", "C++"],
                                   "level": "advanced", "prerequisites": ["파이썬"], "next_topics": []})).unwrap();
        let once = markdown(&stored);
        assert_eq!(once, markdown(&serde_json::from_str(&stored.to_string()).unwrap()));
        assert_eq!(
            once,
            "## 한 줄 요약\n\nC\\#과 \\*포인터\\*를 다룹니다.\n\n## 핵심 내용\n\n- \\- 첫째\n- 1\\. 둘째\n\n## 다루는 주제\n\n- \\_\\_init\\_\\_\n- C++\n\n\
             ## 난이도\n\n고급\n\n## 먼저 알면 좋은 것\n\n- 파이썬\n\n## 다음에 볼 주제\n\n없음\n"
        );
    }
}
