//! A job resumed after a kill that came between recording a call's cost and
//! recording the failure it caused: it is already over its limit, so it fails
//! as `cost_limit` without another paid call (A6, T10).

use super::*;
use crate::{AudioExtractor, Browser, Config, KeyStore, Notifier, Services, Transcriber};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// A Gemini that counts the calls it gets and answers each with an empty object.
fn gemini(calls: Arc<AtomicUsize>) -> String {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let at = server.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in server.incoming() {
            let Ok(s) = s else { continue };
            calls.fetch_add(1, Ordering::SeqCst);
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
            let _ = (&mut r).take(len).read_to_end(&mut Vec::new());
            let body = r#"{"candidates":[{"content":{"parts":[{"text":"{}"}]}}],"usageMetadata":{"promptTokenCount":1,"candidatesTokenCount":1}}"#;
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = (&s).write_all(head.as_bytes());
            let _ = (&s).write_all(body.as_bytes());
        }
    });
    format!("http://{at}")
}

#[test]
fn a_resumed_job_over_its_limit_makes_no_more_calls() {
    let dir = std::env::temp_dir().join(format!("vto-cost-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    {
        // An hour's lecture (a 1,000 KRW limit) that has spent 1,450 KRW, cues still untranslated.
        let s = Store::open(&dir.join("app.sqlite3")).unwrap();
        s.create_job("j", "v", "https://a.example/talk", "other", "Talk", 3_600_000, None, "m").unwrap();
        s.save_region("j", "v", 0, &[(0, 900, "hello".into()), (1000, 1900, "attention".into())]).unwrap();
        s.end_writing("j").unwrap();
        s.add_usage("j", 1, 1, |_, _| 1_450.0).unwrap();
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let svc = Services {
        keystore: Box::new(Fake),
        browser: Box::new(Fake),
        notifier: Box::new(Fake),
        transcriber: Box::new(Fake),
        extractor: Box::new(Fake),
    };
    let core = Core::start(Config { data_dir: dir.clone(), gemini_base_url: gemini(calls.clone()), allow_loopback_media: false }, svc).unwrap();
    let mut j = core.store.job("j").unwrap().unwrap();
    for _ in 0..200 {
        if j.stage == "failed" || j.stage == "ready" {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        j = core.store.job("j").unwrap().unwrap();
    }
    assert_eq!((j.stage.as_str(), j.failure_reason.as_deref()), ("failed", Some("cost_limit")));
    assert_eq!(calls.load(Ordering::SeqCst), 0, "Gemini was called past the limit");
}
