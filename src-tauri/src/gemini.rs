//! The translation module: Gemini's generateContent with numbered segments.

use crate::http::{self, Request};
use serde_json::{json, Value};

pub const MODEL: &str = "gemini-3.1-flash-lite";

// Paid-tier text prices for gemini-3.1-flash-lite (USD per 1M tokens), from
// https://ai.google.dev/gemini-api/docs/pricing; thinking tokens bill as output.
pub const INPUT_USD_PER_MILLION: f64 = 0.25;
pub const OUTPUT_USD_PER_MILLION: f64 = 1.50;
pub const KRW_PER_USD: f64 = 1_450.0;
/// The budget: this many KRW per hour of video, write (on device, free) and translation together.
pub const KRW_PER_HOUR: f64 = 1_000.0;

pub fn cost_krw(input: i64, output: i64) -> f64 {
    (input as f64 * INPUT_USD_PER_MILLION + output as f64 * OUTPUT_USD_PER_MILLION) / 1e6 * KRW_PER_USD
}

pub fn limit_krw(duration_ms: i64) -> f64 {
    duration_ms.max(0) as f64 / 3_600_000.0 * KRW_PER_HOUR
}

pub const BATCH: usize = 40;

const SYSTEM: &str = "You translate subtitles of technical talks and lectures (deep learning, programming, \
science) into natural Korean. Each work order is JSON. For {\"task\":\"glossary\"}, find the technical terms, \
model names and proper nouns in the transcript and give each one fixed Korean rendering; keep product, model \
and code names (such as GPT-4o, PyTorch, ReLU) exactly as written. For {\"task\":\"translate\"}, translate every \
segment on its own, keep its id, never merge, split, drop or reorder segments, and always use the glossary's \
target for its source term. Reply with JSON only: {\"terms\":[{\"source\":\"…\",\"target\":\"…\"}]} for a glossary, \
{\"segments\":[{\"id\":1,\"ko\":\"…\"}]} for a translation.";

#[derive(Debug)]
pub enum Error {
    /// The key was refused (401/403): retrying will not help.
    Key(String),
    /// Network trouble, rate limits or server errors: worth retrying later.
    Transient(String),
    /// A reply that cannot be used, with whatever usage it billed.
    Reply(String, Usage),
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Usage {
    pub input: i64,
    pub output: i64,
}

pub struct Client {
    pub base_url: String,
    pub key: String,
    pub model: String,
}

impl Client {
    fn url(&self, tail: &str) -> String {
        format!("{}/v1beta/models/{}{tail}", self.base_url.trim_end_matches('/'), self.model)
    }

    /// Checks that the key works without spending tokens (a model lookup).
    pub fn check_key(&self) -> Result<(), Error> {
        let r = http::send(Request::get(&self.url("")).header("x-goog-api-key", &self.key)).map_err(|e| Error::Transient(e.0))?;
        match r.status {
            200..=299 => Ok(()),
            400 | 401 | 403 => Err(Error::Key(format!("HTTP {}", r.status))),
            s => Err(Error::Transient(format!("HTTP {s}"))),
        }
    }

    /// Sends one work order and returns the JSON object the model answered.
    pub fn order(&self, order: &Value) -> Result<(Value, Usage), Error> {
        let body = json!({
            "systemInstruction": {"parts": [{"text": SYSTEM}]},
            "contents": [{"role": "user", "parts": [{"text": order.to_string()}]}],
            "generationConfig": {
                "responseMimeType": "application/json",
                "temperature": 0.2,
                // ponytail: minimal thinking keeps output tokens (billed) low; raise it if quality needs it.
                "thinkingConfig": {"thinkingLevel": "minimal"}
            }
        })
        .to_string();
        let r = http::send(Request::post(&self.url(":generateContent"), body.as_bytes()).header("x-goog-api-key", &self.key).header("Content-Type", "application/json"))
            .map_err(|e| Error::Transient(e.0))?;
        let status = r.status;
        let bytes = r.bytes(16 << 20).map_err(|e| Error::Transient(e.0))?;
        match status {
            200..=299 => {}
            401 | 403 => return Err(Error::Key(format!("HTTP {status}"))),
            400 if key_refused(&bytes) => return Err(Error::Key(format!("HTTP {status}"))),
            _ => return Err(Error::Transient(format!("HTTP {status}"))),
        }
        let v: Value = serde_json::from_slice(&bytes).map_err(|e| Error::Transient(format!("reply is not JSON: {e}")))?;
        let u = &v["usageMetadata"];
        let n = |k: &str| u[k].as_i64().unwrap_or(0);
        let usage = Usage { input: n("promptTokenCount"), output: n("candidatesTokenCount") + n("thoughtsTokenCount") };
        let text: String = v["candidates"][0]["content"]["parts"]
            .as_array()
            .map(|ps| ps.iter().filter(|p| p["thought"] != true).filter_map(|p| p["text"].as_str()).collect())
            .unwrap_or_default();
        match parse_json_object(&text) {
            Some(obj) => Ok((obj, usage)),
            None => Err(Error::Reply(format!("reply text is not a JSON object: {:.80}", text), usage)),
        }
    }
}

/// Whether a 400 answer is Google refusing the key: an invalid or expired key
/// comes back as 400 INVALID_ARGUMENT with an `API_KEY_*` reason in its details.
fn key_refused(body: &[u8]) -> bool {
    let v: Value = serde_json::from_slice(body).unwrap_or_default();
    let details = v["error"]["details"].as_array().cloned().unwrap_or_default();
    details.iter().any(|d| d["reason"].as_str().is_some_and(|r| r.starts_with("API_KEY_")))
        || v["error"]["message"].as_str().is_some_and(|m| m.contains("API key"))
}

/// The JSON object in a reply, tolerating a ```json fence around it.
fn parse_json_object(text: &str) -> Option<Value> {
    let t = text.trim();
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).map(|s| s.trim_end().trim_end_matches("```")).unwrap_or(t);
    serde_json::from_str::<Value>(t.trim()).ok().filter(Value::is_object)
}

pub fn glossary_order(title: &str, transcript: &[String]) -> Value {
    json!({"task": "glossary", "title": title, "transcript": transcript})
}

/// The glossary entries of a reply, kept in a stable order so every batch carries the same list.
pub fn parse_terms(reply: &Value) -> Vec<Value> {
    let mut terms: Vec<Value> = reply["terms"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|t| {
            let (s, k) = (t["source"].as_str()?.trim(), t["target"].as_str()?.trim());
            (!s.is_empty() && !k.is_empty()).then(|| json!({"source": s, "target": k}))
        })
        .collect();
    terms.sort_by_key(|t| t["source"].as_str().unwrap_or("").to_lowercase());
    terms.dedup_by_key(|t| t["source"].as_str().unwrap_or("").to_lowercase());
    terms
}

pub fn translate_order(glossary: &[Value], segments: &[(i64, String)]) -> Value {
    let segs: Vec<Value> = segments.iter().map(|(id, text)| json!({"id": id, "text": text})).collect();
    json!({"task": "translate", "glossary": glossary, "segments": segs})
}

/// The `(id, ko)` pairs of a reply that answer ids that were asked for.
pub fn parse_translations(reply: &Value, asked: &[(i64, String)]) -> Vec<(i64, String)> {
    reply["segments"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|s| {
            let id = s["id"].as_i64().or_else(|| s["id"].as_str()?.parse().ok())?;
            let ko = s["ko"].as_str()?.trim();
            (asked.iter().any(|(a, _)| *a == id) && !ko.is_empty()).then(|| (id, ko.to_string()))
        })
        .collect()
}

#[cfg(test)]
#[path = "gemini_key_tests.rs"]
mod key_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_hour_of_budget_is_a_thousand_won() {
        assert!((limit_krw(3_600_000) - 1_000.0).abs() < 1e-9);
        // 1M input + 1M output tokens: (0.25 + 1.5) USD at 1450 KRW.
        assert!((cost_krw(1_000_000, 1_000_000) - 2_537.5).abs() < 1e-9);
    }

    #[test]
    fn replies_are_matched_by_id_only() {
        let asked = vec![(3, "a".to_string()), (4, "b".to_string())];
        let reply = parse_json_object("```json\n{\"segments\":[{\"id\":4,\"ko\":\"나\"},{\"id\":\"3\",\"ko\":\"가\"},{\"id\":9,\"ko\":\"x\"}]}\n```").unwrap();
        let mut got = parse_translations(&reply, &asked);
        got.sort();
        assert_eq!(got, vec![(3, "가".to_string()), (4, "나".to_string())]);
    }

    #[test]
    fn glossary_is_deduplicated_and_sorted() {
        let t = parse_terms(&json!({"terms": [{"source": "Transformer", "target": "트랜스포머"}, {"source": "attention", "target": "어텐션"}, {"source": "transformer", "target": "x"}]}));
        assert_eq!(t, vec![json!({"source": "attention", "target": "어텐션"}), json!({"source": "Transformer", "target": "트랜스포머"})]);
    }
}
