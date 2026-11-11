//! The contract host: the real app (commands, capabilities, SQLite, worker)
//! on Tauri's mock runtime, driven over stdin/stdout by `contract-tests`.
//!
//! Only the device pieces are stand-ins, as `contract-tests/src/lib.rs`
//! describes: a file keystore, a fake webview, a JSON-lines notification shade,
//! a table of YouTube audio URLs, and a fake whisper.cpp.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use vto::media::{refuse, Refusal};
use vto::{commands, gemini, http, AudioExtractor, Browser, Config, Core, KeyStore, Notifier, Segment, Services, Transcriber};

// ---------------------------------------------------------------- stand-ins

struct FileKeyStore(PathBuf);

impl KeyStore for FileKeyStore {
    fn get(&self) -> Result<Option<String>, String> {
        Ok(std::fs::read_to_string(&self.0).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
    }
    fn set(&self, key: &str) -> Result<(), String> {
        std::fs::write(&self.0, key).map_err(|e| e.to_string())
    }
    fn delete(&self) -> Result<(), String> {
        match std::fs::remove_file(&self.0) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

#[derive(Default)]
struct View {
    url: Option<String>,
    overlay: bool,
    cookies: Vec<(String, String, String)>,
}

/// The in-app webview: a URL, whether the overlay is attached, and a cookie jar
/// kept in the webview profile under the data folder.
struct FakeWebview {
    jar: PathBuf,
    view: Mutex<View>,
}

impl FakeWebview {
    fn new(profile: PathBuf) -> Self {
        let jar = profile.join("Cookies.json");
        let cookies = std::fs::read(&jar)
            .ok()
            .and_then(|b| serde_json::from_slice::<Vec<(String, String, String)>>(&b).ok())
            .unwrap_or_default();
        let _ = std::fs::create_dir_all(&profile);
        FakeWebview { jar, view: Mutex::new(View { cookies, ..View::default() }) }
    }

    fn save(&self, v: &View) {
        let _ = std::fs::write(&self.jar, serde_json::to_vec(&v.cookies).unwrap());
    }

    fn set_cookie(&self, url: &str, name: &str, value: &str) -> Result<(), String> {
        let domain = url::Url::parse(url).map_err(|e| e.to_string())?.host_str().unwrap_or("").to_string();
        let mut v = self.view.lock().unwrap();
        v.cookies.retain(|(d, n, _)| !(d == &domain && n == name));
        v.cookies.push((domain, name.into(), value.into()));
        self.save(&v);
        Ok(())
    }

    fn snapshot(&self) -> Value {
        let v = self.view.lock().unwrap();
        let cookies: Vec<Value> = v.cookies.iter().map(|(d, n, val)| json!({"domain": d, "name": n, "value": val})).collect();
        json!({"url": v.url, "overlay": v.overlay, "cookies": cookies})
    }
}

impl Browser for FakeWebview {
    fn navigate(&self, url: &str, overlay: bool) -> Result<(), String> {
        let mut v = self.view.lock().unwrap();
        v.url = Some(url.into());
        v.overlay = overlay;
        Ok(())
    }
    fn clear_site_data(&self) -> Result<(), String> {
        let mut v = self.view.lock().unwrap();
        v.cookies.clear();
        let _ = std::fs::remove_file(&self.jar);
        Ok(())
    }
}

struct Shade(Mutex<PathBuf>);

impl Shade {
    fn put(&self, v: Value) {
        let path = self.0.lock().unwrap();
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&*path).expect("notifications file");
        let _ = writeln!(f, "{v}");
    }
}

impl Notifier for Shade {
    fn ongoing(&self, job_id: &str, state: &str, title: &str, percent: i64) {
        self.put(json!({"kind": "ongoing", "state": state, "job_id": job_id, "title": title, "percent": percent}));
    }
    fn ready(&self, job_id: &str, text: &str, url: &str) {
        self.put(json!({"kind": "ready", "job_id": job_id, "text": text, "url": url}));
    }
}

struct TableExtractor(HashMap<String, String>);

impl AudioExtractor for TableExtractor {
    fn audio_url(&self, page_url: &str) -> Result<String, Refusal> {
        self.0.get(page_url).cloned().ok_or_else(|| refuse("youtube_unavailable", "유튜브에서 이 영상의 오디오를 받을 수 없습니다."))
    }
}

/// whisper.cpp as `contract-tests/src/lib.rs` specifies it.
struct FakeWhisper {
    lines: Vec<String>,
    ms_per_audio_second: u64,
    next: AtomicUsize,
}

impl Transcriber for FakeWhisper {
    fn transcribe(&self, pcm: &[f32], progress: &mut dyn FnMut(f32)) -> Result<Vec<Segment>, String> {
        const F: usize = 160; // 10 ms
        let voiced: Vec<bool> = pcm.chunks(F).map(|f| (f.iter().map(|x| x * x).sum::<f32>() / f.len() as f32).sqrt() >= 0.02).collect();
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for (i, &v) in voiced.iter().enumerate() {
            if !v {
                continue;
            }
            match runs.last_mut() {
                Some((_, e)) if i - *e < 30 => *e = i + 1,
                _ => runs.push((i, i + 1)),
            }
        }
        let mut segs = Vec::new();
        for (a, b) in &runs {
            let text = if self.lines.is_empty() { String::new() } else { self.lines[self.next.fetch_add(1, Ordering::SeqCst) % self.lines.len()].clone() };
            segs.push(Segment { start_ms: (*a * 10) as i64, end_ms: (*b * 10) as i64, text });
        }
        // The hallucination: every unvoiced stretch of 2 s or more.
        let mut i = 0;
        while i < voiced.len() {
            if voiced[i] {
                i += 1;
                continue;
            }
            let start = i;
            while i < voiced.len() && !voiced[i] {
                i += 1;
            }
            if i - start >= 200 {
                segs.push(Segment { start_ms: (start * 10) as i64, end_ms: (i * 10) as i64, text: "Thank you for watching.".into() });
            }
        }
        segs.sort_by_key(|s| s.start_ms);
        let total = Duration::from_millis(pcm.len() as u64 * self.ms_per_audio_second / 16_000);
        let begin = Instant::now();
        while begin.elapsed() < total {
            progress(begin.elapsed().as_secs_f32() / total.as_secs_f32());
            std::thread::sleep(Duration::from_millis(20).min(total.saturating_sub(begin.elapsed())));
        }
        progress(1.0);
        Ok(segs)
    }
}

// ---------------------------------------------------------------- host

/// The data folder this host may own. The harness names an app's folder
/// `vto-contract-{pid}-{nanos}`; when two tests of one test process read the
/// same clock tick, their hosts get the same folder. A host that finds the
/// folder held by another live host therefore moves into its own sub-folder,
/// which is still under `data_dir` where the tests look. The lock dies with
/// the process, so a restarted host takes back the folder its predecessor had.
fn claim_data_dir(base: &std::path::Path) -> PathBuf {
    let _ = std::fs::create_dir_all(base);
    for n in 0..16 {
        let lock = base.join(format!("host-{n}.lock"));
        let Ok(f) = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&lock) else { continue };
        if f.try_lock().is_ok() {
            std::mem::forget(f); // held until the process ends
            let dir = if n == 0 { base.to_path_buf() } else { base.join(format!("instance-{n}")) };
            if n > 0 {
                vto::log(format!("data folder shared with another live host; using {}", dir.display()));
            }
            return dir;
        }
    }
    base.to_path_buf()
}

fn main() {
    let path = std::env::var("VTO_CONTRACT_CONFIG").expect("VTO_CONTRACT_CONFIG");
    let cfg: Value = serde_json::from_slice(&std::fs::read(&path).expect("read the host config")).expect("host config JSON");
    let s = |k: &str| PathBuf::from(cfg[k].as_str().unwrap_or_else(|| panic!("config.{k}")));
    let data_dir = claim_data_dir(&s("data_dir"));
    let webview = Arc::new(FakeWebview::new(data_dir.join("webview")));
    let youtube: HashMap<String, String> = serde_json::from_value(cfg["youtube_audio"].clone()).unwrap_or_default();
    let t = &cfg["transcriber"];
    let lines: Vec<String> = serde_json::from_value(t["lines"].clone()).unwrap_or_default();

    struct SharedView(Arc<FakeWebview>);
    impl Browser for SharedView {
        fn navigate(&self, url: &str, overlay: bool) -> Result<(), String> {
            self.0.navigate(url, overlay)
        }
        fn clear_site_data(&self) -> Result<(), String> {
            self.0.clear_site_data()
        }
    }

    let core = Core::start(
        Config {
            data_dir,
            gemini_base_url: cfg["gemini_base_url"].as_str().unwrap_or(vto::GEMINI_BASE_URL).to_string(),
            allow_loopback_media: cfg["allow_loopback_media"].as_bool().unwrap_or(false),
        },
        Services {
            keystore: Box::new(FileKeyStore(s("keystore_path"))),
            browser: Box::new(SharedView(webview.clone())),
            notifier: Box::new(Shade(Mutex::new(s("notifications_path")))),
            transcriber: Box::new(FakeWhisper { lines, ms_per_audio_second: t["ms_per_audio_second"].as_u64().unwrap_or(100), next: AtomicUsize::new(0) }),
            extractor: Box::new(TableExtractor(youtube)),
        },
    )
    .expect("start the app core");

    let mut ctx = mock_context(noop_assets());
    *ctx.runtime_authority_mut() = commands::authority();
    let app = commands::register(mock_builder(), core).build(ctx).expect("build the mock app");
    let main = WebviewWindowBuilder::new(&app, "main", WebviewUrl::App("index.html".into())).build().expect("main window");
    let site = WebviewWindowBuilder::new(&app, "site", WebviewUrl::External("https://example.invalid/".parse().unwrap()))
        .build()
        .expect("site window");
    let local_url: url::Url = if cfg!(windows) { "http://tauri.localhost/" } else { "tauri://localhost/" }.parse().unwrap();

    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        let resp = match req["op"].as_str().unwrap_or("") {
            "call" => {
                let origin = req["origin"].as_str().unwrap_or("app");
                let (window, url) = if origin == "app" {
                    (&main, local_url.clone())
                } else {
                    (&site, origin.parse().unwrap_or_else(|_| "https://invalid.example/".parse().unwrap()))
                };
                let method = req["method"].as_str().unwrap_or("");
                let path = req["path"].as_str().unwrap_or("");
                match commands::route(method, path, &req["body"]) {
                    None => json!({"status": 404, "body": {"error": "no_route", "message": format!("{method} {path}")}}),
                    Some((cmd, args)) => {
                        let r = get_ipc_response(
                            window,
                            InvokeRequest {
                                cmd: cmd.into(),
                                callback: CallbackFn(0),
                                error: CallbackFn(1),
                                url,
                                body: InvokeBody::Json(args),
                                headers: Default::default(),
                                invoke_key: INVOKE_KEY.into(),
                            },
                        );
                        match r {
                            Ok(body) => json!({"status": 200, "body": body.deserialize::<Value>().unwrap_or(Value::Null)}),
                            Err(e) => match e["status"].as_u64() {
                                Some(status) => json!({"status": status, "body": e}),
                                None => json!({"status": 403, "body": {"error": "forbidden", "message": e}}),
                            },
                        }
                    }
                }
            }
            "webview" => webview.snapshot(),
            "set_cookie" => match webview.set_cookie(req["url"].as_str().unwrap_or(""), req["name"].as_str().unwrap_or(""), req["value"].as_str().unwrap_or("")) {
                Ok(()) => json!({"ok": true}),
                Err(e) => json!({"ok": false, "error": e}),
            },
            "commands" => json!({"commands": commands::COMMANDS}),
            "network" => {
                http::block_network(req["blocked"].as_bool().unwrap_or(true));
                json!({"ok": true})
            }
            "pricing" => json!({
                "input_usd_per_million": gemini::INPUT_USD_PER_MILLION,
                "output_usd_per_million": gemini::OUTPUT_USD_PER_MILLION,
                "krw_per_usd": gemini::KRW_PER_USD,
            }),
            other => json!({"error": format!("unknown op {other:?}")}),
        };
        let _ = writeln!(out, "{resp}");
        let _ = out.flush();
    }
}
