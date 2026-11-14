//! The worker: the ongoing notice's progress (R24), the ready notice of a job
//! resumed after the process was killed (R5, T8), and a job that hits a bug.

use super::*;
use crate::{AudioExtractor, Browser, Config, KeyStore, Notifier, Services, Transcriber};
use std::sync::Mutex;
use serde_json::json;
use std::io::{BufRead, BufReader, Read, Write};

#[derive(Default, Clone)]
struct Shade {
    updates: Arc<Mutex<Vec<i64>>>,
    ready: Arc<Mutex<Vec<String>>>,
}
impl Notifier for Shade {
    fn ongoing(&self, _: &str, state: &str, _: &str, percent: i64) {
        if state == "updated" {
            self.updates.lock().unwrap().push(percent);
        }
    }
    fn ready(&self, job_id: &str, _: &str, _: &str) {
        self.ready.lock().unwrap().push(job_id.into());
    }
}
/// A stored key, or a key store with a bug in it.
struct Keys {
    broken: bool,
}
impl KeyStore for Keys {
    fn get(&self) -> Result<Option<String>, String> {
        assert!(!self.broken, "a bug in the key store");
        Ok(Some("AIzaSyD-test-key-0123".into()))
    }
    fn set(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn delete(&self) -> Result<(), String> {
        Ok(())
    }
}
struct Nothing;
impl Browser for Nothing {
    fn navigate(&self, _: &str, _: bool) -> Result<(), String> {
        Ok(())
    }
    fn clear_site_data(&self) -> Result<(), String> {
        Ok(())
    }
}
impl Transcriber for Nothing {
    fn transcribe(&self, _: &[f32], _: &mut dyn FnMut(f32)) -> Result<Vec<crate::Segment>, String> {
        Ok(Vec::new())
    }
}
impl AudioExtractor for Nothing {
    fn audio_url(&self, _: &str) -> Result<String, media::Refusal> {
        Err(media::refuse("youtube_unavailable", ""))
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("vto-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start(dir: &std::path::Path, shade: &Shade, broken_keys: bool) -> Arc<Core> {
    Core::start(
        Config { data_dir: dir.into(), gemini_base_url: String::new(), allow_loopback_media: false },
        Services {
            keystore: Box::new(Keys { broken: broken_keys }),
            browser: Box::new(Nothing),
            notifier: Box::new(shade.clone()),
            transcriber: Box::new(Nothing),
            extractor: Box::new(Nothing),
        },
    )
    .unwrap()
}

/// What a killed process leaves when every cue was translated but the ready
/// notice had not gone out yet.
fn translated_but_not_notified(dir: &std::path::Path) {
    let s = Store::open(&dir.join("app.sqlite3")).unwrap();
    s.create_job("j", "v", "https://a.example/talk", "other", "Talk", 1000, None, "m").unwrap();
    s.save_region("j", "v", 0, &[(0, 900, "hello".into())]).unwrap();
    s.end_writing("j").unwrap();
    s.set_terms("v", "[]").unwrap();
    s.set_translations("j", &[(0, "안녕하세요".into())]).unwrap();
}

fn settled(core: &Core) -> Job {
    for _ in 0..200 {
        let j = core.store.job("j").unwrap().unwrap();
        if j.stage == "ready" || j.stage == "failed" {
            return j;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("the job never settled: {:?}", core.store.job("j"));
}

#[test]
fn every_ten_percent_step_passed_is_reported_once() {
    let dir = temp_dir("progress");
    let shade = Shade::default();
    let core = start(&dir, &shade, false);
    let cx = Ctx { core: &core, store: &core.store, title: "t".into(), page_url: String::new() };
    for (stage, end) in [(Stage::DOWNLOADING, 10), (Stage::WRITING, 90), (Stage::TRANSLATING, 100)] {
        let mut last = Stage::new(stage);
        for p in [0, 3, 13, 14, 27, 26, 61, 100] {
            cx.percent("no-such-job", p, &mut last);
        }
        assert_eq!(shade.updates.lock().unwrap().last(), Some(&end), "a finished stage ends at its share of the whole job");
    }
    // Writing: 13 passes 10; 27 passes 20; 61 passes 30-60 at once; 100 passes the rest.
    // The notice shows the whole job, so it never runs back when a stage starts.
    let seen = shade.updates.lock().unwrap();
    assert_eq!(seen[4..8], [20, 31, 58, 90]);
    assert!(seen.windows(2).all(|w| w[0] <= w[1]), "the notice ran back: {seen:?}");
}

#[test]
fn a_job_resumed_before_its_notice_went_out_notifies_once() {
    let dir = temp_dir("resume-notice");
    translated_but_not_notified(&dir);
    let shade = Shade::default();
    let core = start(&dir, &shade, false);
    let j = settled(&core);
    assert_eq!((j.stage.as_str(), j.notified), ("ready", true));
    assert_eq!(*shade.ready.lock().unwrap(), ["j"]);
}

#[test]
fn a_bug_fails_the_job_instead_of_leaving_it_running() {
    let dir = temp_dir("panic");
    translated_but_not_notified(&dir);
    let shade = Shade::default();
    let core = start(&dir, &shade, true);
    let j = settled(&core);
    assert_eq!((j.stage.as_str(), j.failure_reason.as_deref()), ("failed", Some("translate_failed")));
    assert!(core.store.active_job().unwrap().is_none(), "the failed job still holds the worker");
    assert!(shade.ready.lock().unwrap().is_empty());
}

fn idle_core(dir: &Path, endpoint: &str) -> Core {
    Core {
        updates: Default::default(),
        cfg: Config { data_dir: dir.into(), gemini_base_url: endpoint.into(), allow_loopback_media: true },
        store: Store::open(&dir.join("app.sqlite3")).unwrap(),
        svc: Services {
            keystore: Box::new(Keys { broken: false }),
            browser: Box::new(Nothing),
            notifier: Box::new(Shade::default()),
            transcriber: Box::new(Nothing),
            extractor: Box::new(Nothing),
        },
        wake: Default::default(),
        create_lock: Default::default(),
    }
}

fn local_gemini(mut answer: impl FnMut(Value) -> (u16, Value, i64) + Send + 'static) -> String {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let at = server.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in server.incoming() {
            let mut stream = stream.unwrap();
            let mut r = BufReader::new(stream.try_clone().unwrap());
            let mut len = 0;
            loop {
                let mut line = String::new();
                r.read_line(&mut line).unwrap();
                if line.trim().is_empty() { break; }
                if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = n.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; len];
            r.read_exact(&mut body).unwrap();
            let body: Value = serde_json::from_slice(&body).unwrap();
            let order = serde_json::from_str(body["contents"][0]["parts"][0]["text"].as_str().unwrap()).unwrap();
            let (status, text, input) = answer(order);
            let body = json!({
                "candidates": [{"content": {"parts": [{"text": text.to_string()}]}}],
                "usageMetadata": {"promptTokenCount": input, "candidatesTokenCount": 1}
            }).to_string();
            let head = format!("HTTP/1.1 {status} Reply\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(body.as_bytes());
        }
    });
    format!("http://{at}")
}

const SAVED_PAGE: &str = "https://a.example/saved";

fn saved_subtitles(core: &Core) -> Value {
    let key = api::video_key(&url::Url::parse(SAVED_PAGE).unwrap());
    let s = &core.store;
    s.create_job("original", &key, SAVED_PAGE, "other", "Saved speech", 3_600_000, None, "m").unwrap();
    s.save_region("original", &key, 0, &[(0, 900, "first".into()), (1000, 1900, "second".into())]).unwrap();
    s.set_terms(&key, "[]").unwrap();
    s.set_translations("original", &[(0, "original first".into()), (1, "original second".into())]).unwrap();
    s.add_usage("original", 10, 10, gemini::cost_krw).unwrap();
    s.finish("original").unwrap();
    api::get_subtitles(core, &key).unwrap()
}

#[test]
fn failed_retranslation_preserves_completed_subtitles_for_every_stop() {
    // Failure must never publish a partial replacement or hide the completed set.
    for (case, status, text, input, reason) in [
        ("transient", 503, json!({}), 1, "translate_failed"),
        ("unusable", 200, json!({}), 1, "translate_failed"),
        ("key", 403, json!({}), 1, "key_rejected"),
        ("cost", 200, json!({"segments":[{"id":1,"ko":"replacement second"}]}), 40_000_000, "cost_limit"),
    ] {
        let endpoint = local_gemini(move |_| (status, text.clone(), input));
        let dir = temp_dir(&format!("retranslation-{case}"));
        let core = idle_core(&dir, &endpoint);
        let before = saved_subtitles(&core);
        let key = before["video_key"].as_str().unwrap();
        let retry = api::retranslate_job(&core, SAVED_PAGE).unwrap();
        let id = retry["job_id"].as_str().unwrap();
        core.store.set_translations(id, &[(0, "replacement first".into())]).unwrap();
        assert_eq!(api::get_subtitles(&core, key).unwrap(), before, "{case}: active replacement leaked");
        drop(core);

        let core = idle_core(&dir, &endpoint);
        run_job(&core, &core.store, core.store.job(id).unwrap().unwrap());
        let failed = core.store.job(id).unwrap().unwrap();
        assert_eq!((failed.stage.as_str(), failed.failure_reason.as_deref()), ("failed", Some(reason)), "{case}");
        assert_eq!(api::get_subtitles(&core, key).unwrap(), before, "{case}: saved subtitles lost");
        assert_eq!(core.store.cues(key).unwrap()[0].text_ko.as_deref(), Some("original first"));
        assert_eq!(core.store.job("original").unwrap().unwrap().stage, "ready");
        let reused = api::create_job(&core, SAVED_PAGE, None, "Saved", 3_600_000).unwrap();
        assert_eq!(reused["job_id"], "original", "{case}: detecting again discarded the saved set");
        assert_eq!(reused["reused"], true);
        let next = api::retranslate_job(&core, SAVED_PAGE);
        if case == "cost" {
            assert_eq!(next.unwrap_err().error, "cost_limit");
        } else {
            let next = next.unwrap();
            let next = core.store.job(next["job_id"].as_str().unwrap()).unwrap().unwrap();
            assert_eq!(next.cost_krw, failed.cost_krw, "a retry forgot earlier usage");
        }
    }
}

#[test]
fn retranslation_resumes_partial_work_and_publishes_only_a_complete_set() {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let log = asked.clone();
    let endpoint = local_gemini(move |order| {
        let id = order["segments"][0]["id"].as_i64().unwrap();
        log.lock().unwrap().push(id);
        (200, json!({"segments":[{"id":id,"ko":"replacement second"}]}), 1)
    });
    let dir = temp_dir("retranslation-publish");
    let core = idle_core(&dir, &endpoint);
    let before = saved_subtitles(&core);
    let key = before["video_key"].as_str().unwrap();
    let retry = api::retranslate_job(&core, SAVED_PAGE).unwrap();
    let id = retry["job_id"].as_str().unwrap();
    core.store.set_translations(id, &[(0, "replacement first".into())]).unwrap();
    assert!(core.store.finish(id).is_err(), "an incomplete replacement was published");
    assert_eq!(api::get_subtitles(&core, key).unwrap(), before);
    drop(core);

    let core = idle_core(&dir, &endpoint);
    run_job(&core, &core.store, core.store.job(id).unwrap().unwrap());
    assert_eq!(core.store.job(id).unwrap().unwrap().stage, "ready");
    let after = api::get_subtitles(&core, key).unwrap();
    assert_eq!(after["cues"][0]["text_ko"], "replacement first");
    assert_eq!(after["cues"][1]["text_ko"], "replacement second");
    for i in 0..2 {
        for field in ["seq", "start_ms", "end_ms"] {
            assert_eq!(after["cues"][i][field], before["cues"][i][field]);
        }
    }
    assert_eq!(*asked.lock().unwrap(), [1], "restart repeated already completed work");
}

#[test]
fn transient_retries_are_independent_for_glossary_and_each_cue() {
    let counts = Arc::new(Mutex::new(std::collections::HashMap::<String, usize>::new()));
    let log = counts.clone();
    let endpoint = local_gemini(move |order| {
        let key = if order["task"] == "glossary" { "glossary".into() } else { order["segments"][0]["id"].to_string() };
        let mut counts = log.lock().unwrap();
        let count = counts.entry(key).or_default();
        *count += 1;
        if *count == 1 { return (503, json!({}), 0); }
        let text = if order["task"] == "glossary" {
            json!({"terms":[]})
        } else {
            json!({"segments":[{"id":order["segments"][0]["id"],"ko":"translated"}]})
        };
        (200, text, 1)
    });
    let dir = temp_dir("per-request-retries");
    let core = idle_core(&dir, &endpoint);
    let s = &core.store;
    s.create_job("j", "v", SAVED_PAGE, "other", "Lecture", 3_600_000, None, "m").unwrap();
    let cues: Vec<_> = (0..9).map(|i| (i * 1000, i * 1000 + 900, format!("cue {i}"))).collect();
    s.save_region("j", "v", 0, &cues).unwrap();
    s.end_writing("j").unwrap();
    run_job(&core, s, s.job("j").unwrap().unwrap());
    assert_eq!(s.job("j").unwrap().unwrap().stage, "ready");
    assert!(s.cues("v").unwrap().iter().all(|c| c.text_ko.as_deref() == Some("translated")));
    let counts = counts.lock().unwrap();
    assert_eq!(counts.len(), 10);
    assert!(counts.values().all(|n| *n == 2), "{counts:?}");
    assert_eq!(s.job("j").unwrap().unwrap().input_amount, 10, "successful calls were not metered");
}
