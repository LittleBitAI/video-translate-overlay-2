//! The app in a browser: the built screens and the API on one port.
//!
//! `PORT` is the port (on 127.0.0.1); `APP_DATA_DIR` holds all the app's data
//! (SQLite, downloads, the model); `VTO_DIST` the built screens (default: the
//! repository's `dist/`, from `npm run build`). `scripts/live.mjs` builds both
//! and starts it.
//!
//! The site a person opens is shown by the first screen itself, so the "browser"
//! here only accepts the address (P1 records it for P9). Notices are the first
//! screen's: it polls P7. The on-device model is in the build with the
//! `whisper` feature; without it a job stops at writing as `download_failed`.

use std::path::PathBuf;
use std::sync::Arc;
#[cfg(not(feature = "whisper"))]
use vto::Segment;
use vto::{youtube, Browser, Config, Core, Notifier, Services, Transcriber};

struct Page;

impl Browser for Page {
    fn navigate(&self, _url: &str, _overlay: bool) -> Result<(), String> {
        Ok(())
    }
    /// The browser's cookies are its own: only the YouTube account screen is reset (by P2).
    fn clear_site_data(&self) -> Result<(), String> {
        Ok(())
    }
}

struct Log;

impl Notifier for Log {
    fn ongoing(&self, job_id: &str, state: &str, _title: &str, percent: i64) {
        vto::log(format!("job {job_id}: {state} {percent}%"));
    }
    fn ready(&self, job_id: &str, _text: &str, _url: &str) {
        vto::log(format!("job {job_id}: subtitles ready"));
    }
}

#[cfg(not(feature = "whisper"))]
struct NoModel;

#[cfg(not(feature = "whisper"))]
impl Transcriber for NoModel {
    fn prepare(&self, _: &std::path::Path, _: &mut dyn FnMut(f32)) -> Result<bool, String> {
        Err("this build has no on-device model (build vto-web with --features whisper)".into())
    }
    fn transcribe(&self, _: &[f32], _: &mut dyn FnMut(f32)) -> Result<Vec<Segment>, String> {
        Err("no on-device model".into())
    }
}

fn main() {
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).expect("PORT: the port to serve on");
    let data_dir = PathBuf::from(std::env::var_os("APP_DATA_DIR").expect("APP_DATA_DIR: the app's data folder"));
    std::fs::create_dir_all(&data_dir).expect("create APP_DATA_DIR");
    let dist = std::env::var_os("VTO_DIST").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist"));
    assert!(dist.join("index.html").is_file(), "no built screens at {} (npm run build)", dist.display());

    #[cfg(feature = "whisper")]
    let transcriber: Box<dyn Transcriber> = Box::new(vto::whisper::Whisper::new(&data_dir, std::env::var_os("VTO_WHISPER_MODEL").map(Into::into)));
    #[cfg(not(feature = "whisper"))]
    let transcriber: Box<dyn Transcriber> = Box::new(NoModel);

    let core = Core::start(
        Config { data_dir: data_dir.clone(), gemini_base_url: vto::GEMINI_BASE_URL.into(), allow_loopback_media: false },
        Services {
            keystore: Box::new(vto::platform::CredentialStore::for_folder(&data_dir)),
            browser: Box::new(Page),
            notifier: Box::new(Log),
            transcriber,
            extractor: Box::new(youtube::InnertubeExtractor),
        },
    )
    .expect("start the app core");
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).expect("bind 127.0.0.1:PORT");
    vto::log(format!("serving http://127.0.0.1:{port}/ from {}", dist.display()));
    vto::web::serve(listener, Arc::clone(&core), dist);
}
