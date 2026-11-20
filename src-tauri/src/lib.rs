//! video-translate-overlay backend: Tauri commands P1-P11 (also over HTTP, `web`), the background job
//! pipeline (download → write on device → translate with Gemini → notify),
//! and the device services those need.

pub mod api;
pub mod account;
#[cfg(windows)]
pub mod account_secrets;
#[cfg(all(feature = "app", target_os = "android"))]
pub mod android;
#[cfg(feature = "app")]
pub mod app;
pub mod audio;
pub mod commands;
pub mod db;
// MPEG-TS everywhere; every other format off Windows.
mod decode;
pub mod gemini;
pub mod google_auth;
pub mod google_desktop;
mod google_registration;
pub mod http;
pub mod media;
pub mod pipeline;
pub mod sentences;
pub mod summary;
pub mod platform;
pub mod screens;
pub mod stitch;
pub mod store;
mod library_scope;
pub mod sync;
mod sync_tls;
mod sync_channel;
mod sync_discovery;
mod sync_runtime;
pub mod updates;
pub mod web;
#[cfg(feature = "whisper")]
pub mod whisper;
pub mod youtube;

pub const PRIVACY_POLICY_URL: &str = "https://littlebitai.github.io/video-translate-overlay-2/privacy.html";

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// A piece of written speech, timed from the start of the PCM handed to the model.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    /// The segment's words in order, timed the same way, when the model gives
    /// them; their texts joined are `text`. Empty: the segment is one unit.
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Word {
    pub start_ms: i64,
    pub end_ms: i64,
    /// With the whitespace that comes before it.
    pub text: String,
}

/// The on-device speech model (whisper.cpp in the app).
pub trait Transcriber: Send + Sync {
    /// Makes sure the model file is on this device under `data_dir`, fetching it
    /// if not; returns whether it had to be fetched. `progress` gets 0.0..=1.0.
    fn prepare(&self, _data_dir: &std::path::Path, _progress: &mut dyn FnMut(f32)) -> Result<bool, String> {
        Ok(false)
    }
    /// Whether `prepare` would fetch the model, known before it connects.
    fn needs_fetch(&self) -> bool {
        false
    }
    /// `pcm` is 16 kHz mono; `progress` receives 0.0..=1.0 as the model works.
    /// Loads the model into memory ahead of the first [`Transcriber::transcribe`].
    fn load(&self) -> Result<(), String> {
        Ok(())
    }
    fn transcribe(&self, pcm: &[f32], progress: &mut dyn FnMut(f32)) -> Result<Vec<Segment>, String>;
    /// Text that tells the model what the talk is about (its title), for the
    /// windows transcribed after it; `None` clears it.
    fn set_hint(&self, _hint: Option<&str>) {}
    /// Frees the loaded model (memory, and on a GPU its video memory); the
    /// next [`Transcriber::load`] reads it again.
    fn unload(&self) {}
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
    fn refresh_library(&self) -> Result<(), String> { Ok(()) }
    fn invalidate_account(&self, _generation: u64) -> Result<(), String> { Ok(()) }
    fn open_download(&self, _url: &str) -> Result<(), String> { Err("external downloads are unavailable".into()) }
    fn open_privacy_policy(&self) -> Result<(), String> { self.open_download(PRIVACY_POLICY_URL) }
    /// Drops cached subtitles in open views before permanent cleanup starts.
    fn invalidate_video(&self, _key: &str) -> Result<(), String> { Ok(()) }
    /// Shows `url`; `overlay` attaches the overlay script (never on Google's sign-in pages).
    fn navigate(&self, url: &str, overlay: bool) -> Result<(), String>;
    /// Deletes cookies and site storage.
    fn clear_site_data(&self) -> Result<(), String>;
}

/// The notification shade.
pub trait Notifier: Send + Sync {
    fn register_sync(&self, _owner: &str) -> Result<(), String> { Ok(()) }
    fn sync_background(&self, _owner: &str, _generation: u64, _enabled: bool) -> Result<bool, String> { Ok(true) }
    /// The ongoing notice of a running job: `shown`, `updated` or `hidden`.
    fn ongoing(&self, job_id: &str, state: &str, title: &str, percent: i64);
    /// Called when 영상감지 is pressed, before the media check, while the app is
    /// still on screen: Android lets a foreground service start only then (R6). An
    /// error refuses the job. The `ongoing` `shown` that follows a created job
    /// only updates it; `hidden` lets it go when the job is refused instead.
    fn hold(&self, _job_id: &str, _title: &str) -> Result<(), String> {
        Ok(())
    }
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
    pub(crate) sync: sync_runtime::StatusStore,
    pub account: account::Manager,
    pub updates: updates::Checker,
    pub cfg: Config,
    pub store: store::Store,
    pub svc: Services,
    wake: (Mutex<bool>, Condvar),
    /// Held while P6 checks for existing jobs and creates one.
    pub(crate) create_lock: Mutex<()>,
    pub(crate) running: Running,
}

/// What only this process knows about the job being started or run; a restart forgets it.
#[derive(Default)]
pub(crate) struct Running {
    /// The source P6 just probed for this job, handed to its first download attempt.
    pub(crate) probed: Mutex<Option<(String, media::Source)>>,
    /// The job whose download stage is fetching the speech model right now.
    pub(crate) preparing_model: Mutex<Option<String>>,
}

impl Core {
    /// Opens the data folder with no worker: nothing runs a job until the
    /// caller does. A tool that runs one named job itself
    /// (`examples/remake_saved.rs`) opens this, so no job it did not name,
    /// in the data folder or an account library below it, is run or paid for.
    pub fn open(cfg: Config, svc: Services) -> Result<Arc<Core>, String> {
        std::fs::create_dir_all(cfg.data_dir.join("audio")).map_err(|e| format!("data folder: {e}"))?;
        let store = store::Store::open(&cfg.data_dir.join("app.sqlite3"))?;
        let account = account::Manager::new(cfg.data_dir.clone());
        Ok(Arc::new(Core { cfg, store, svc, account, sync: Default::default(), updates: updates::Checker::default(), wake: (Mutex::new(false), Condvar::new()), create_lock: Mutex::new(()), running: Running::default() }))
    }

    /// Opens the data folder and starts the worker, which first resumes unfinished jobs.
    pub fn start(cfg: Config, svc: Services) -> Result<Arc<Core>, String> {
        let core = Self::open(cfg, svc)?;
        let worker = core.clone();
        std::thread::Builder::new().name("jobs".into()).spawn(move || pipeline::run_worker(worker)).map_err(|e| e.to_string())?;
        Ok(core)
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
    // An app's stderr goes nowhere on Android; logcat (tag `vto`) is where its log is read.
    #[cfg(target_os = "android")]
    {
        #[link(name = "log")]
        extern "C" {
            fn __android_log_write(prio: i32, tag: *const std::ffi::c_char, text: *const std::ffi::c_char) -> i32;
        }
        if let Ok(text) = std::ffi::CString::new(msg.as_ref().replace('\0', " ")) {
            // 4 = ANDROID_LOG_INFO.
            unsafe { __android_log_write(4, c"vto".as_ptr(), text.as_ptr()) };
        }
    }
}
