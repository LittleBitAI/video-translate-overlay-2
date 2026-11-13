//! The glossary comes first and every batch carries it (A6, R9): a reply that
//! is not a glossary is asked for again, and a job that never gets one fails
//! as `translate_failed` instead of translating without it.

use super::*;
use crate::{AudioExtractor, Browser, Config, KeyStore, Notifier, Services, Transcriber};
use std::io::{BufRead, BufReader, Read, Write};
use serde_json::json;
use std::sync::Mutex;

struct Fake;
impl KeyStore for Fake {
    fn get(&self) -> Result<Option<String>, String> {
        Ok(Some("AIzaSyD-test-key-0123".into()))
    }
    fn set(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn delete(&self) -> Result<(), String> {
        Ok(())
    }
}
impl Browser for Fake {
    fn navigate(&self, _: &str, _: bool) -> Result<(), String> {
        Ok(())
    }
    fn clear_site_data(&self) -> Result<(), String> {
        Ok(())
    }
}
impl Notifier for Fake {
    fn ongoing(&self, _: &str, _: &str, _: &str, _: i64) {}
    fn ready(&self, _: &str, _: &str, _: &str) {}
}
impl Transcriber for Fake {
    fn transcribe(&self, _: &[f32], _: &mut dyn FnMut(f32)) -> Result<Vec<crate::Segment>, String> {
        Ok(Vec::new())
    }
}
impl AudioExtractor for Fake {
    fn audio_url(&self, _: &str) -> Result<String, media::Refusal> {
        Err(media::refuse("youtube_unavailable", ""))
    }
}

/// What the fake Gemini was asked: the glossary orders it got, and the
/// glossary each translate order carried.
#[derive(Default)]
struct Asked {
    glossaries: usize,
    carried: Vec<Value>,
}

/// A Gemini whose glossary replies are `bad` unusable ones (prose, then an
/// object with no term list) and then a real one; translations always come back.
fn gemini(bad: usize, asked: Arc<Mutex<Asked>>) -> String {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let at = server.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in server.incoming() {
            let Ok(s) = s else { continue };
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut len = 0u64;
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                    break;
                }
                if let Some(v) = l.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = Vec::new();
            let _ = (&mut r).take(len).read_to_end(&mut body);
            let body: Value = serde_json::from_slice(&body).unwrap_or_default();
            let order: Value = body["contents"][0]["parts"][0]["text"].as_str().and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default();
            let text = {
                let mut a = asked.lock().unwrap();
                if order["task"] == "glossary" {
                    a.glossaries += 1;
                    match a.glossaries {
                        n if n > bad => json!({"terms": [{"source": "attention", "target": "어텐션"}]}).to_string(),
                        1 => "Sorry, here are the terms: attention".into(),
                        _ => "{}".into(),
                    }
                } else {
                    a.carried.push(order["glossary"].clone());
                    let segs: Vec<Value> = order["segments"].as_array().cloned().unwrap_or_default().iter().map(|s| json!({"id": s["id"], "ko": "번역"})).collect();
                    json!({ "segments": segs }).to_string()
                }
            };
            let reply = json!({"candidates": [{"content": {"parts": [{"text": text}]}}], "usageMetadata": {"promptTokenCount": 1, "candidatesTokenCount": 1}}).to_string();
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", reply.len());
            let _ = (&s).write_all(head.as_bytes());
            let _ = (&s).write_all(reply.as_bytes());
        }
    });
    format!("http://{at}")
}

/// Runs a written, untranslated job against a Gemini with `bad` unusable glossary replies.
fn run(name: &str, bad: usize) -> (Job, Asked) {
    let dir = std::env::temp_dir().join(format!("vto-glossary-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    {
        let s = Store::open(&dir.join("app.sqlite3")).unwrap();
        s.create_job("j", "v", "https://a.example/talk", "other", "Talk", 3_600_000, None, "m").unwrap();
        s.save_region("j", "v", 0, &[(0, 900, "hello".into()), (1000, 1900, "attention".into())]).unwrap();
        s.end_writing("j").unwrap();
    }
    let asked = Arc::new(Mutex::new(Asked::default()));
    let svc = Services {
        keystore: Box::new(Fake),
        browser: Box::new(Fake),
        notifier: Box::new(Fake),
        transcriber: Box::new(Fake),
        extractor: Box::new(Fake),
    };
    let core = Core::start(Config { data_dir: dir, gemini_base_url: gemini(bad, asked.clone()), allow_loopback_media: false }, svc).unwrap();
    let mut j = core.store.job("j").unwrap().unwrap();
    for _ in 0..400 {
        if j.stage == "failed" || j.stage == "ready" {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        j = core.store.job("j").unwrap().unwrap();
    }
    let a = std::mem::take(&mut *asked.lock().unwrap());
    (j, a)
}

#[test]
fn an_unusable_glossary_reply_is_asked_for_again() {
    let (j, a) = run("again", 2);
    assert_eq!(j.stage, "ready", "{:?}", j.failure_reason);
    assert_eq!(a.glossaries, 3);
    assert!(!a.carried.is_empty());
    for g in &a.carried {
        assert_eq!(g, &json!([{"source": "attention", "target": "어텐션"}]), "a batch went without the glossary");
    }
}

#[test]
fn a_job_with_no_glossary_fails_before_any_batch() {
    let (j, a) = run("never", usize::MAX);
    assert_eq!((j.stage.as_str(), j.failure_reason.as_deref()), ("failed", Some("translate_failed")));
    assert_eq!(a.glossaries, GLOSSARY_TRIES);
    assert!(a.carried.is_empty(), "translated without a glossary: {:?}", a.carried);
}
