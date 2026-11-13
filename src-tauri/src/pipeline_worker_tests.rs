//! The worker: the ongoing notice's progress (R24), the ready notice of a job
//! resumed after the process was killed (R5, T8), and a job that hits a bug.

use super::*;
use crate::{AudioExtractor, Browser, Config, KeyStore, Notifier, Services, Transcriber};
use std::sync::Mutex;

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
    s.set_translations("v", &[(0, "안녕하세요".into())]).unwrap();
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
