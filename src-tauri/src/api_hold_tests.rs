//! 영상감지 holds the background service before the media check, which can
//! take tens of seconds: Android starts a foreground service only while the
//! app is on screen (R6). A refused hold refuses the job; a refused check lets
//! the hold go.

use super::*;
use crate::{AudioExtractor, Browser, Config, KeyStore, Notifier, Services, Transcriber};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Rec {
    log: Arc<Mutex<Vec<String>>>,
    hold_refused: bool,
    audio: Option<String>,
}

impl Rec {
    fn put(&self, s: &str) {
        self.log.lock().unwrap().push(s.into());
    }
}
impl KeyStore for Rec {
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
impl Browser for Rec {
    fn navigate(&self, _: &str, _: bool) -> Result<(), String> {
        Ok(())
    }
    fn clear_site_data(&self) -> Result<(), String> {
        Ok(())
    }
}
impl Notifier for Rec {
    fn hold(&self, _: &str, _: &str) -> Result<(), String> {
        self.put("hold");
        if self.hold_refused {
            Err("ForegroundServiceStartNotAllowedException".into())
        } else {
            Ok(())
        }
    }
    fn ongoing(&self, _: &str, state: &str, _: &str, _: i64) {
        self.put(state);
    }
    fn ready(&self, _: &str, _: &str, _: &str) {}
}
impl Transcriber for Rec {
    fn transcribe(&self, _: &[f32], _: &mut dyn FnMut(f32)) -> Result<Vec<crate::Segment>, String> {
        Ok(Vec::new())
    }
}
impl AudioExtractor for Rec {
    fn audio_url(&self, _: &str) -> Result<String, media::Refusal> {
        self.put("check");
        self.audio.clone().ok_or_else(|| media::refuse("youtube_unavailable", "x"))
    }
}

fn press(name: &str, hold_refused: bool, audio: Option<String>) -> (Result<Value, ApiError>, Vec<String>, bool) {
    let dir = std::env::temp_dir().join(format!("vto-hold-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rec = Rec { log: Arc::default(), hold_refused, audio };
    let svc = Services {
        keystore: Box::new(rec.clone()),
        browser: Box::new(rec.clone()),
        notifier: Box::new(rec.clone()),
        transcriber: Box::new(rec.clone()),
        extractor: Box::new(rec.clone()),
    };
    let core = Core::start(Config { data_dir: dir, gemini_base_url: String::new(), allow_loopback_media: true }, svc).unwrap();
    let r = create_job(&core, "https://www.youtube.com/watch?v=dQw4w9WgXcQ", None, "A lecture", 60_000);
    let active = core.store.active_job().unwrap().is_some();
    let log = rec.log.lock().unwrap().clone();
    (r, log, active)
}

#[test]
fn the_service_is_held_before_the_media_check() {
    let (r, log, active) = press("ok", false, Some(format!("{}/videoplayback?itag=140", super::duration_tests::serve_wav())));
    assert_eq!(r.unwrap()["stage"], "downloading");
    assert_eq!(log[..3], ["hold", "check", "shown"], "{log:?}");
    assert!(active);
}

#[test]
fn a_refused_hold_refuses_the_job_before_any_check() {
    let (r, log, active) = press("refused", true, Some("http://unused".into()));
    let e = r.unwrap_err();
    assert_eq!((e.status, e.error.as_str()), (409, "background_refused"));
    assert_eq!(log, ["hold"]);
    assert!(!active);
}

#[test]
fn a_refused_check_lets_the_hold_go() {
    let (r, log, active) = press("unavailable", false, None);
    assert_eq!(r.unwrap_err().error, "youtube_unavailable");
    assert_eq!(log, ["hold", "check", "hidden"]);
    assert!(!active);
}
