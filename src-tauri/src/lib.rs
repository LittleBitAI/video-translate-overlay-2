//! video-translate-overlay backend: Tauri commands P1-P8, the background job
//! pipeline (download → write on device → translate with Gemini → notify),
//! and the device services those need.

pub mod api;
pub mod audio;
pub mod commands;
pub mod db;
pub mod gemini;
pub mod http;
pub mod media;
pub mod pipeline;
pub mod platform;
pub mod store;
pub mod youtube;

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// A piece of written speech, timed from the start of the PCM handed to the model.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// The on-device speech model (whisper.cpp in the app).
pub trait Transcriber: Send + Sync {
    /// `pcm` is 16 kHz mono; `progress` receives 0.0..=1.0 as the model works.
    fn transcribe(&self, pcm: &[f32], progress: &mut dyn FnMut(f32)) -> Result<Vec<Segment>, String>;
    /// The language the model heard, when it knows.
    fn language(&self) -> Option<String> {
        None
    }
}

/// The device's secure store for the Gemini key.
pub trait KeyStore: Send + Sync {
    fn get(&self) -> Result<Option<String>, String>;
    fn set(&self, key: &str) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

/// The in-app webview that shows video sites.
pub trait Browser: Send + Sync {
    /// Shows `url`; `overlay` attaches the overlay script (never on Google's sign-in pages).
    fn navigate(&self, url: &str, overlay: bool) -> Result<(), String>;
    /// Deletes cookies and site storage.
    fn clear_site_data(&self) -> Result<(), String>;
}

/// The notification shade.
pub trait Notifier: Send + Sync {
    /// The ongoing notice of a running job: `shown`, `updated` or `hidden`.
    fn ongoing(&self, job_id: &str, state: &str, title: &str, percent: i64);
    /// The notice that subtitles are ready; tapping it opens `url`.
    fn ready(&self, job_id: &str, text: &str, url: &str);
}

/// Finds the audio stream of a YouTube page.
pub trait AudioExtractor: Send + Sync {
    fn audio_url(&self, page_url: &str) -> Result<String, media::Refusal>;
}

pub struct Services {
    pub keystore: Box<dyn KeyStore>,
    pub browser: Box<dyn Browser>,
    pub notifier: Box<dyn Notifier>,
    pub transcriber: Box<dyn Transcriber>,
    pub extractor: Box<dyn AudioExtractor>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub gemini_base_url: String,
    /// Lets media come from this device (the contract host's media server only).
    pub allow_loopback_media: bool,
}

pub const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";

/// Everything the commands and the worker share.
pub struct Core {
    pub cfg: Config,
    pub store: store::Store,
    pub svc: Services,
    wake: (Mutex<bool>, Condvar),
}

impl Core {
    /// Opens the data folder and starts the worker, which first resumes unfinished jobs.
    pub fn start(cfg: Config, svc: Services) -> Result<Arc<Core>, String> {
        std::fs::create_dir_all(cfg.data_dir.join("audio")).map_err(|e| format!("data folder: {e}"))?;
        let store = store::Store::open(&cfg.data_dir.join("app.sqlite3"))?;
        let core = Arc::new(Core { cfg, store, svc, wake: (Mutex::new(false), Condvar::new()) });
        let worker = core.clone();
        std::thread::Builder::new().name("jobs".into()).spawn(move || pipeline::run_worker(worker)).map_err(|e| e.to_string())?;
        Ok(core)
    }

    pub fn db_path(&self) -> PathBuf {
        self.cfg.data_dir.join("app.sqlite3")
    }

    pub fn wake_worker(&self) {
        *self.wake.0.lock().unwrap() = true;
        self.wake.1.notify_all();
    }

    pub(crate) fn wait_for_work(&self, timeout: Duration) {
        let mut woken = self.wake.0.lock().unwrap();
        if !*woken {
            woken = self.wake.1.wait_timeout(woken, timeout).unwrap().0;
        }
        *woken = false;
    }
}

pub fn log(msg: impl AsRef<str>) {
    eprintln!("[vto] {}", msg.as_ref());
}
