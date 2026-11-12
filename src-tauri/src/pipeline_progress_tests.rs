//! The ongoing notice's progress (R24): one update per 10% step passed, even
//! when the percent jumps over the multiples of ten, as a share of the whole job.

use super::*;
use crate::{AudioExtractor, Browser, Config, KeyStore, Notifier, Services, Transcriber};
use std::sync::Mutex;

struct Shade(Arc<Mutex<Vec<i64>>>);
impl Notifier for Shade {
    fn ongoing(&self, _: &str, state: &str, _: &str, percent: i64) {
        if state == "updated" {
            self.0.lock().unwrap().push(percent);
        }
    }
    fn ready(&self, _: &str, _: &str, _: &str) {}
}
struct Nothing;
impl KeyStore for Nothing {
    fn get(&self) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn set(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn delete(&self) -> Result<(), String> {
        Ok(())
    }
}
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

#[test]
fn every_ten_percent_step_passed_is_reported_once() {
    let dir = std::env::temp_dir().join(format!("vto-progress-{}-{:?}", std::process::id(), std::thread::current().id()));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let core = Core::start(
        Config { data_dir: dir.clone(), gemini_base_url: String::new(), allow_loopback_media: false },
        Services {
            keystore: Box::new(Nothing),
            browser: Box::new(Nothing),
            notifier: Box::new(Shade(seen.clone())),
            transcriber: Box::new(Nothing),
            extractor: Box::new(Nothing),
        },
    )
    .unwrap();
    let cx = Ctx { core: &core, store: &core.store, title: "t".into(), page_url: String::new() };
    for (stage, end) in [(Stage::DOWNLOADING, 10), (Stage::WRITING, 90), (Stage::TRANSLATING, 100)] {
        let mut last = Stage::new(stage);
        for p in [0, 3, 13, 14, 27, 26, 61, 100] {
            cx.percent("no-such-job", p, &mut last);
        }
        assert_eq!(seen.lock().unwrap().last(), Some(&end), "a finished stage ends at its share of the whole job");
    }
    // Writing: 13 passes 10; 27 passes 20; 61 passes 30-60 at once; 100 passes the rest.
    // The notice shows the whole job, so it never runs back when a stage starts.
    let seen = seen.lock().unwrap();
    assert_eq!(seen[4..8], [20, 31, 58, 90]);
    assert!(seen.windows(2).all(|w| w[0] <= w[1]), "the notice ran back: {seen:?}");
    let _ = std::fs::remove_dir_all(dir);
}
