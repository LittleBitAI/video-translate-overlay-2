//! Translation requests run side by side, and each is still one cue with the
//! glossary and only the cue before it (A6, R9); the cost limit still stops
//! new requests, with no more than one call past it, as one at a time did.

use super::*;
use serde_json::json;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A Gemini that answers every connection on its own thread after `delay_ms`,
/// keeping each order it got and the most it was answering at once. Every
/// reply bills `input` prompt tokens.
fn gemini(delay_ms: u64, input: i64, orders: Arc<Mutex<Vec<Value>>>, most: Arc<AtomicUsize>) -> String {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let at = server.local_addr().unwrap();
    let now = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for stream in server.incoming() {
            let (orders, most, now) = (orders.clone(), most.clone(), now.clone());
            std::thread::spawn(move || {
                let mut stream = stream.unwrap();
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    r.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = n.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; len];
                r.read_exact(&mut body).unwrap();
                let body: Value = serde_json::from_slice(&body).unwrap();
                let order: Value = serde_json::from_str(body["contents"][0]["parts"][0]["text"].as_str().unwrap()).unwrap();
                most.fetch_max(now.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(delay_ms));
                now.fetch_sub(1, Ordering::SeqCst);
                let text = if order["task"] == "glossary" {
                    json!({"terms": [{"source": "attention", "target": "어텐션"}]})
                } else {
                    let id = &order["segments"][0]["id"];
                    json!({"segments": [{"id": id, "ko": format!("번역 {id}")}]})
                };
                orders.lock().unwrap().push(order);
                let body = json!({
                    "candidates": [{"content": {"parts": [{"text": text.to_string()}]}}],
                    "usageMetadata": {"promptTokenCount": input, "candidatesTokenCount": 1}
                })
                .to_string();
                let head = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body.as_bytes());
            });
        }
    });
    format!("http://{at}")
}

fn dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("vto-translate-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A job past writing with `n` cues, "cue 0" to "cue n-1", for an hour of video (a 1,000 KRW limit).
fn written(core: &Core, n: i64) {
    let s = &core.store;
    s.create_job("j", "v", "https://a.example/talk", "other", "Lecture", 3_600_000, None, "m").unwrap();
    let cues: Vec<_> = (0..n).map(|i| (i * 1000, i * 1000 + 900, format!("cue {i}"))).collect();
    s.save_region("j", "v", 0, &cues, None).unwrap();
    s.end_writing("j").unwrap();
}

#[test]
fn cues_are_translated_one_per_request_with_requests_overlapping() {
    let (orders, most) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicUsize::new(0)));
    let core = super::worker_tests::idle_core(&dir("overlap"), &gemini(150, 10, orders.clone(), most.clone()));
    written(&core, 24);
    run_job(&core, &core.store, core.store.job("j").unwrap().unwrap());

    assert_eq!(core.store.job("j").unwrap().unwrap().stage, "ready");
    for c in core.store.cues("v").unwrap() {
        assert_eq!(c.text_ko, Some(format!("번역 {}", c.seq)));
    }
    let orders = orders.lock().unwrap();
    let translations: Vec<&Value> = orders.iter().filter(|o| o["task"] == "translate").collect();
    assert_eq!(translations.len(), 24);
    for o in &translations {
        let segs = o["segments"].as_array().unwrap();
        assert_eq!(segs.len(), 1, "a request with more than one cue: {o}");
        let id = segs[0]["id"].as_i64().unwrap();
        assert_eq!(segs[0]["text"], format!("cue {id}"));
        let before = (id > 0).then(|| format!("cue {}", id - 1));
        assert_eq!(o["context_before"].as_str().map(String::from), before, "the context is not the cue before: {o}");
        assert_eq!(o["glossary"], json!([{"source": "attention", "target": "어텐션"}]));
    }
    let most = most.load(Ordering::SeqCst);
    assert!((4..=IN_FLIGHT).contains(&most), "{most} requests at once");
    // Every call metered once: the glossary's and each cue's.
    assert_eq!(core.store.job("j").unwrap().unwrap().input_amount, 250);
}

#[test]
fn the_limit_stops_new_requests_while_others_are_in_flight() {
    // 100,000 prompt tokens a call: 36.25 KRW, so the 1,000 KRW limit holds 27 calls.
    let (orders, most) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicUsize::new(0)));
    let core = super::worker_tests::idle_core(&dir("limit"), &gemini(50, 100_000, orders.clone(), most.clone()));
    written(&core, 60);
    run_job(&core, &core.store, core.store.job("j").unwrap().unwrap());

    let job = core.store.job("j").unwrap().unwrap();
    assert_eq!((job.stage.as_str(), job.failure_reason.as_deref()), ("failed", Some("cost_limit")));
    let call = gemini::cost_krw(100_000, 1);
    // As with one call at a time: at most the one call that crossed the limit is past it.
    assert!(job.cost_krw <= 1_000.0 + call + 1e-6, "spent {} KRW", job.cost_krw);
    assert_eq!(orders.lock().unwrap().len(), (job.cost_krw / call).round() as usize, "a call was not metered");
    assert!(most.load(Ordering::SeqCst) >= 2, "the requests never overlapped");
}
