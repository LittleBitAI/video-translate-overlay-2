//! Harness for the backend contract tests.
//!
//! The tests never link the app. Each test starts the app's contract host, a
//! binary named `vto-contract-host` that the app crate provides, and talks to it
//! over stdin/stdout. `scripts/contract-tests.mjs` builds that binary and passes
//! its path in `VTO_CONTRACT_HOST`; when it is missing every test fails.
//!
//! # What the contract host is
//!
//! The real app (Tauri commands, SQLite, background jobs, capabilities) running
//! on Tauri's mock runtime, with only these stand-ins:
//!
//! * the secure store (Windows Credential Manager / Android Keystore) is a file
//!   at `keystore_path`;
//! * the in-app webview is a fake that records the URL it shows, whether the
//!   overlay script is attached, and keeps its cookies under `data_dir`;
//! * the notification shade is a JSON-lines file at `notifications_path`;
//! * the YouTube audio extractor is the `youtube_audio` table (a YouTube
//!   `page_url` not in the table is not downloadable);
//! * whisper.cpp with its model already present in `data_dir` is the fake
//!   transcriber described under `transcriber` below.
//!
//! Everything else, including Gemini calls (sent to `gemini_base_url`), media
//! downloads (HTTP from this harness), voice activity detection, time mapping,
//! cost accounting and resuming after a restart, is the app's own code.
//!
//! # Start-up
//!
//! The host reads the JSON file named by `VTO_CONTRACT_CONFIG` before it does
//! anything else, including resuming unfinished jobs:
//!
//! ```json
//! {
//!   "data_dir": "…",            // app data folder: SQLite, webview profile, logs
//!   "keystore_path": "…",
//!   "notifications_path": "…",
//!   "gemini_base_url": "http://127.0.0.1:PORT",
//!   "allow_loopback_media": true, // lets media come from this harness
//!   "youtube_audio": {"https://www.youtube.com/watch?v=…": "http://127.0.0.1:PORT/…"},
//!   "transcriber": {"lines": ["…"], "ms_per_audio_second": 100}
//! }
//! ```
//!
//! # Fake transcriber
//!
//! It is handed mono f32 PCM at 16 kHz and returns segments timed from the start
//! of the PCM it was handed. Frames are 10 ms; a frame is voiced when its RMS is
//! at least 0.02. Voiced frames closer than 300 ms form one segment, whose text
//! is the next entry of `lines` (cycling). Like the real model it hallucinates:
//! every unvoiced stretch of 2 s or more inside its input yields a segment
//! "Thank you for watching." covering that stretch. It takes
//! `ms_per_audio_second` of wall time per second of input and reports progress
//! through that time the way whisper.cpp's progress callback does.
//!
//! # Protocol
//!
//! One JSON object per line each way; every request gets exactly one response
//! line on stdout, in order. Logs go to stderr.
//!
//! * `{"op":"call","method":"POST","path":"/jobs","body":{…},"origin":"app"}`
//!   → `{"status":200,"body":{…}}`. `method` and `path` are those of
//!   `docs/design/api.yaml`; the host routes them to the matching Tauri command
//!   through the IPC with the capability checks of the real app. `origin` is
//!   `"app"` for the app's own screen, or the URL of the site page the webview
//!   shows when the call comes from that page (the overlay). A call the
//!   capabilities refuse answers with a status of 400 or more.
//! * `{"op":"webview"}` → `{"url": "…"|null, "overlay": bool,
//!   "cookies": [{"domain":"…","name":"…","value":"…"}]}`.
//! * `{"op":"set_cookie","url":"…","name":"…","value":"…"}` → `{"ok":true}`:
//!   what Google's sign-in page leaves in the webview when the user signs in.
//! * `{"op":"page_loaded","url":"…"}` → `{"ok":true}`: a native page-load callback.
//! * `{"op":"commands"}` → `{"commands":["…"]}`: every registered command.
//! * `{"op":"network","blocked":true}` → `{"ok":true}`: from now on every
//!   outbound connection the app tries fails.
//! * `{"op":"pricing"}` → `{"input_usd_per_million":…,
//!   "output_usd_per_million":…, "krw_per_usd":…}`: the constants the app
//!   charges Gemini usage with.
//!
//! # Notification records
//!
//! * `{"kind":"ongoing","state":"shown"|"updated"|"hidden","job_id":"…"}`
//! * `{"kind":"ready","job_id":"…","text":"…","url":"…"}`: `text` is everything
//!   the notification shows, `url` the page it opens.
//!
//! # Gemini wire format (served by [`FakeGemini`])
//!
//! `POST {gemini_base_url}/v1beta/models/{model}:generateContent` with the key
//! in `x-goog-api-key` (or `?key=`). The work order is the last text part in
//! `contents` that is a JSON object with a `task` field:
//!
//! * `{"task":"glossary", …the whole transcript…}` → the reply text is
//!   `{"terms":[{"source":"…","target":"…"}]}`;
//! * `{"task":"translate","glossary":[{"source","target"}],
//!   "segments":[{"id":1,"text":"…"}]}` → `{"segments":[{"id":1,"ko":"…"}]}`.
//!
//! Replies carry `usageMetadata` with `promptTokenCount`,
//! `candidatesTokenCount` and `thoughtsTokenCount`; thoughts bill as output.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub use serde_json;

/// A Gemini key shaped like a real one; its last four characters are `WXYZ`.
pub const KEY: &str = "AIzaSyA1b2C3d4E5f6G7h8I9j0K1l2M3n4OWXYZ";
pub const MODEL: &str = "gemini-3.1-flash-lite";
pub const STAGES: [&str; 4] = ["downloading", "writing", "translating", "ready"];
pub const APP: &str = "app";

pub fn sleep_ms(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

// ---------------------------------------------------------------- audio

pub const RATE: u32 = 16_000;

/// A 16 kHz mono PCM16 WAV of `total_ms`, digital silence except for a
/// synthetic voice (140 Hz with harmonics) over each `(start_ms, end_ms)` span.
pub fn speech_wav(total_ms: u64, spans: &[(u64, u64)]) -> Vec<u8> {
    let n = (total_ms * RATE as u64 / 1000) as usize;
    let mut out = Vec::with_capacity(44 + n * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + n as u32 * 2).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(n as u32 * 2).to_le_bytes());
    out.resize(44 + n * 2, 0);
    let fade = (RATE / 100) as f64;
    for &(a, b) in spans {
        let i0 = (a * RATE as u64 / 1000) as usize;
        let i1 = ((b * RATE as u64 / 1000) as usize).min(n);
        for i in i0..i1 {
            let t = i as f64 / RATE as f64;
            let mut v = 0.0;
            for k in 1..=5 {
                v += (2.0 * std::f64::consts::PI * 140.0 * k as f64 * t).sin() / k as f64;
            }
            let edge = ((i - i0) as f64 / fade).min((i1 - i) as f64 / fade).min(1.0);
            let env = edge * (0.85 + 0.15 * (2.0 * std::f64::consts::PI * 4.0 * t).sin());
            let s = (0.3 * env * v / 2.2833 * 32767.0).round() as i16;
            out[44 + i * 2..46 + i * 2].copy_from_slice(&s.to_le_bytes());
        }
    }
    out
}

/// `count` spans of `speech_ms`, one every `every_ms`, starting at `first_ms`.
pub fn regular_spans(count: u64, first_ms: u64, every_ms: u64, speech_ms: u64) -> Vec<(u64, u64)> {
    (0..count)
        .map(|i| (first_ms + i * every_ms, first_ms + i * every_ms + speech_ms))
        .collect()
}

// ---------------------------------------------------------------- tiny HTTP server

pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Clone, Default)]
pub struct Gate(Arc<(Mutex<bool>, Condvar)>);

impl Gate {
    fn wait(&self) {
        let (open, cv) = &*self.0;
        let mut g = open.lock().unwrap();
        while !*g {
            g = cv.wait(g).unwrap();
        }
    }
    fn open(&self) {
        let (open, cv) = &*self.0;
        *open.lock().unwrap() = true;
        cv.notify_all();
    }
}

pub struct HttpResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Arc<Vec<u8>>,
    /// Send this many body bytes, then wait for the gate before the rest.
    pub hold: Option<(usize, Gate)>,
}

impl HttpResponse {
    pub fn new(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        HttpResponse { status, content_type: content_type.into(), body: Arc::new(body), hold: None }
    }
    pub fn json(status: u16, v: &Value) -> Self {
        Self::new(status, "application/json", v.to_string().into_bytes())
    }
}

type Handler = Arc<dyn Fn(&HttpRequest) -> HttpResponse + Send + Sync>;

pub struct HttpServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl HttpServer {
    pub fn start(handler: Handler) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = thread::spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let h = handler.clone();
                        thread::spawn(move || {
                            let _ = serve_connection(stream, &h);
                        });
                    }
                    Err(_) => sleep_ms(5),
                }
            }
        });
        HttpServer { addr, stop, thread: Some(thread) }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Closes the listening socket: later connections are refused.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn serve_connection(stream: TcpStream, handler: &Handler) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let mut req = HttpRequest { method, path, headers, body: Vec::new() };
    if let Some(len) = req.header("content-length").and_then(|v| v.parse::<usize>().ok()) {
        let mut body = vec![0; len];
        reader.read_exact(&mut body)?;
        req.body = body;
    } else if req.header("transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked")) {
        loop {
            let mut size = String::new();
            reader.read_line(&mut size)?;
            let n = usize::from_str_radix(size.trim().split(';').next().unwrap_or("0"), 16).unwrap_or(0);
            let mut chunk = vec![0; n + 2];
            reader.read_exact(&mut chunk)?;
            if n == 0 {
                break;
            }
            req.body.extend_from_slice(&chunk[..n]);
        }
    }
    let resp = handler(&req);
    let mut out = stream;
    let full = resp.body.as_slice();
    let (mut status, mut body) = (resp.status, full);
    let mut extra = String::new();
    if resp.status == 200 {
        if let Some((a, b)) = req.header("range").and_then(|r| parse_range(r, full.len())) {
            status = 206;
            body = &full[a..=b];
            extra = format!("Content-Range: bytes {a}-{b}/{}\r\n", full.len());
        }
    }
    let reason = match status {
        200 => "OK",
        206 => "Partial Content",
        404 => "Not Found",
        _ => "Status",
    };
    write!(
        out,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n{extra}Connection: close\r\n\r\n",
        resp.content_type,
        body.len()
    )?;
    if req.method == "HEAD" {
        return out.flush();
    }
    match resp.hold {
        Some((first, gate)) if body.len() > first => {
            out.write_all(&body[..first])?;
            out.flush()?;
            gate.wait();
            out.write_all(&body[first..])?;
        }
        _ => out.write_all(body)?,
    }
    out.flush()
}

fn parse_range(header: &str, len: usize) -> Option<(usize, usize)> {
    let spec = header.trim().strip_prefix("bytes=")?;
    let (a, b) = spec.split_once('-')?;
    if len == 0 {
        return None;
    }
    if a.is_empty() {
        let n: usize = b.parse().ok()?;
        return Some((len.saturating_sub(n), len - 1));
    }
    let a: usize = a.parse().ok()?;
    let b: usize = if b.is_empty() { len - 1 } else { b.parse::<usize>().ok()?.min(len - 1) };
    (a <= b).then_some((a, b))
}

// ---------------------------------------------------------------- media server

#[derive(Default)]
struct MediaState {
    files: HashMap<String, (String, Arc<Vec<u8>>)>,
    holds: HashMap<String, Gate>,
    hits: Vec<String>,
}

/// Serves audio files and HLS playlists the app downloads.
pub struct MediaServer {
    server: HttpServer,
    state: Arc<Mutex<MediaState>>,
}

impl MediaServer {
    pub fn start() -> Self {
        let state = Arc::new(Mutex::new(MediaState::default()));
        let s = state.clone();
        let server = HttpServer::start(Arc::new(move |req: &HttpRequest| {
            let path = req.path.split('?').next().unwrap_or("").to_string();
            let mut st = s.lock().unwrap();
            st.hits.push(format!("{} {}", req.method, path));
            let Some((ct, bytes)) = st.files.get(&path).cloned() else {
                return HttpResponse::new(404, "text/plain", b"not found".to_vec());
            };
            let hold = st.holds.get(&path).cloned();
            drop(st);
            let mut resp = HttpResponse { status: 200, content_type: ct, body: bytes.clone(), hold: None };
            // Small range probes are answered at once; a real download stalls part way.
            let small_range = req
                .header("range")
                .and_then(|r| parse_range(r, bytes.len()))
                .is_some_and(|(a, b)| b - a < 65_536);
            if let (Some(gate), false) = (hold, small_range) {
                resp.hold = Some(((bytes.len() / 8).min(65_536), gate));
            }
            resp
        }));
        MediaServer { server, state }
    }

    pub fn serve(&self, path: &str, content_type: &str, bytes: Vec<u8>) -> String {
        self.state.lock().unwrap().files.insert(path.into(), (content_type.into(), Arc::new(bytes)));
        self.url(path)
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.server.base_url(), path)
    }

    /// Downloads of `path` stall after their first bytes until [`release`](Self::release).
    pub fn hold(&self, path: &str) {
        self.state.lock().unwrap().holds.insert(path.into(), Gate::default());
    }

    pub fn release(&self, path: &str) {
        if let Some(g) = self.state.lock().unwrap().holds.remove(path) {
            g.open();
        }
    }

    pub fn hits(&self) -> Vec<String> {
        self.state.lock().unwrap().hits.clone()
    }

    pub fn stop(&mut self) {
        let gates: Vec<Gate> = self.state.lock().unwrap().holds.drain().map(|(_, g)| g).collect();
        gates.iter().for_each(Gate::open);
        self.server.stop();
    }
}

// ---------------------------------------------------------------- fake Gemini

#[derive(Clone, Debug)]
pub struct GeminiCall {
    pub method: String,
    pub path: String,
    pub model: Option<String>,
    pub api_key: Option<String>,
    /// `glossary`, `translate`, or `None` for anything else (such as a key check).
    pub task: Option<String>,
    pub order: Option<Value>,
    pub prompt_tokens: u64,
    /// Candidates plus thoughts: everything billed at the output rate.
    pub output_tokens: u64,
}

#[derive(Clone, Copy, Default)]
pub struct GeminiOptions {
    pub delay_ms: u64,
    /// Every reply reports an absurd usage (40M prompt, 10M output tokens).
    pub huge_usage: bool,
}

/// Terms the fake recognises: how it renders each one when the request's
/// glossary names it, and the renderings it falls back to otherwise, varying
/// from batch to batch the way an unguided model drifts.
const TERMS: [(&str, &str, [&str; 3]); 4] = [
    ("attention", "어텐션", ["주의", "집중", "주목"]),
    ("transformer", "트랜스포머", ["변환기", "변압기", "변형기"]),
    ("gradient descent", "경사 하강법", ["기울기 내리기", "경사 내림", "구배 하강"]),
    ("GPT-4o", "GPT-4o", ["지피티 4오", "GPT 포오", "지피티-포오"]),
];

pub struct FakeGemini {
    server: HttpServer,
    calls: Arc<Mutex<Vec<GeminiCall>>>,
}

impl FakeGemini {
    pub fn start(opts: GeminiOptions) -> Self {
        let calls: Arc<Mutex<Vec<GeminiCall>>> = Arc::default();
        let log = calls.clone();
        let server = HttpServer::start(Arc::new(move |req: &HttpRequest| {
            let resp = gemini_reply(req, opts, &log);
            if opts.delay_ms > 0 {
                sleep_ms(opts.delay_ms);
            }
            resp
        }));
        FakeGemini { server, calls }
    }

    pub fn base_url(&self) -> String {
        self.server.base_url()
    }

    pub fn calls(&self) -> Vec<GeminiCall> {
        self.calls.lock().unwrap().clone()
    }

    pub fn generate_calls(&self) -> Vec<GeminiCall> {
        self.calls().into_iter().filter(|c| c.path.contains(":generateContent")).collect()
    }

    pub fn task_calls(&self) -> Vec<GeminiCall> {
        self.calls().into_iter().filter(|c| c.task.is_some()).collect()
    }

    pub fn stop(&mut self) {
        self.server.stop();
    }
}

fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    hay.to_ascii_lowercase().find(&needle.to_ascii_lowercase())
}

fn replace_ci(hay: &str, needle: &str, with: &str) -> String {
    let mut out = String::new();
    let mut rest = hay;
    while let Some(i) = find_ci(rest, needle) {
        out.push_str(&rest[..i]);
        out.push_str(with);
        rest = &rest[i + needle.len()..];
    }
    out.push_str(rest);
    out
}

fn gemini_reply(req: &HttpRequest, opts: GeminiOptions, log: &Mutex<Vec<GeminiCall>>) -> HttpResponse {
    let path = req.path.clone();
    let model = path
        .split("/models/")
        .nth(1)
        .map(|m| m.split([':', '?']).next().unwrap_or("").to_string());
    let api_key = req
        .header("x-goog-api-key")
        .map(str::to_string)
        .or_else(|| path.split("key=").nth(1).map(|k| k.split('&').next().unwrap_or("").to_string()));
    let mut call = GeminiCall {
        method: req.method.clone(),
        path: path.clone(),
        model: model.clone(),
        api_key,
        task: None,
        order: None,
        prompt_tokens: 0,
        output_tokens: 0,
    };
    if req.method != "POST" || !path.contains(":generateContent") {
        log.lock().unwrap().push(call);
        let name = format!("models/{}", model.unwrap_or_else(|| MODEL.into()));
        return HttpResponse::json(200, &json!({"name": name, "models": [{"name": format!("models/{MODEL}")}]}));
    }
    let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
    let texts = |v: &Value| -> Vec<String> {
        let mut out = Vec::new();
        let contents = v.as_array().cloned().unwrap_or_else(|| vec![v.clone()]);
        for c in contents {
            for p in c["parts"].as_array().cloned().unwrap_or_default() {
                if let Some(t) = p["text"].as_str() {
                    out.push(t.to_string());
                }
            }
        }
        out
    };
    let parts = texts(&body["contents"]);
    let system = texts(&body["systemInstruction"]);
    let prompt_chars: usize = parts.iter().chain(system.iter()).map(|t| t.chars().count()).sum();
    let order = parts
        .iter()
        .rev()
        .filter_map(|t| serde_json::from_str::<Value>(t.trim()).ok())
        .find(|v| v.get("task").is_some());
    let task = order.as_ref().and_then(|o| o["task"].as_str()).map(str::to_string);
    let mut guard = log.lock().unwrap();
    let batch = guard.iter().filter(|c| c.task.as_deref() == Some("translate")).count();
    let reply_text = match (task.as_deref(), &order) {
        (Some("glossary"), Some(o)) => {
            let all = o.to_string();
            let terms: Vec<Value> = TERMS
                .iter()
                .filter(|(src, _, _)| find_ci(&all, src).is_some())
                .map(|(src, ko, _)| json!({"source": src, "target": ko}))
                .collect();
            json!({ "terms": terms }).to_string()
        }
        (Some("translate"), Some(o)) => {
            let glossary: Vec<(String, String)> = o["glossary"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|g| Some((g["source"].as_str()?.to_string(), g["target"].as_str()?.to_string())))
                .collect();
            let segs: Vec<Value> = o["segments"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|s| {
                    let mut text = s["text"].as_str().unwrap_or("").to_string();
                    for (src, _, drift) in TERMS {
                        if find_ci(&text, src).is_some() {
                            let given = glossary.iter().find(|(g, _)| g.eq_ignore_ascii_case(src));
                            let with = given.map(|(_, t)| t.as_str()).unwrap_or(drift[batch % drift.len()]);
                            text = replace_ci(&text, src, with);
                        }
                    }
                    json!({"id": s["id"].clone(), "ko": format!("한국어 번역: {text}")})
                })
                .collect();
            json!({ "segments": segs }).to_string()
        }
        _ => "OK".to_string(),
    };
    let (prompt, candidates, thoughts) = if opts.huge_usage {
        (40_000_000u64, 10_000_000u64, 0u64)
    } else {
        let p = prompt_chars.div_ceil(4) as u64;
        let c = reply_text.chars().count().div_ceil(3) as u64;
        (p, c, c.div_ceil(10))
    };
    call.task = task;
    call.order = order;
    call.prompt_tokens = prompt;
    call.output_tokens = candidates + thoughts;
    guard.push(call);
    drop(guard);
    HttpResponse::json(
        200,
        &json!({
            "candidates": [{"content": {"role": "model", "parts": [{"text": reply_text}]}, "finishReason": "STOP", "index": 0}],
            "usageMetadata": {
                "promptTokenCount": prompt,
                "candidatesTokenCount": candidates,
                "thoughtsTokenCount": thoughts,
                "totalTokenCount": prompt + candidates + thoughts
            },
            "modelVersion": model.unwrap_or_default()
        }),
    )
}

// ---------------------------------------------------------------- contract host

pub struct Host {
    child: Child,
    stdin: ChildStdin,
    lines: mpsc::Receiver<String>,
}

impl Host {
    fn spawn(config: &Path, stderr_log: &Path, transcript: Arc<Mutex<Vec<String>>>) -> Host {
        let exe = std::env::var("VTO_CONTRACT_HOST").unwrap_or_default();
        assert!(
            !exe.is_empty() && Path::new(&exe).is_file(),
            "the contract host `vto-contract-host` is not built (VTO_CONTRACT_HOST={exe:?}); \
             scripts/contract-tests.mjs builds it with `cargo build --workspace --bin vto-contract-host`"
        );
        let log = fs::OpenOptions::new().create(true).append(true).open(stderr_log).unwrap();
        let mut child = Command::new(&exe)
            .env("VTO_CONTRACT_CONFIG", config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap_or_else(|e| panic!("start {exe}: {e}"));
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        let seen = transcript.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                seen.lock().unwrap().push(line.clone());
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Host { child, stdin, lines: rx }
    }

    fn request(&mut self, req: Value) -> Value {
        writeln!(self.stdin, "{req}").expect("write to the contract host");
        self.stdin.flush().unwrap();
        let line = self
            .lines
            .recv_timeout(Duration::from_secs(60))
            .unwrap_or_else(|e| panic!("no answer from the contract host to {req}: {e}"));
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("contract host answered {line:?} to {req}: {e}"))
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.kill();
    }
}

#[derive(Debug, Clone)]
pub struct Resp {
    pub status: u16,
    pub body: Value,
}

impl Resp {
    pub fn str(&self, field: &str) -> String {
        self.body[field].as_str().unwrap_or_else(|| panic!("{field} missing in {}", self.body)).to_string()
    }
}

#[derive(Clone)]
pub struct Options {
    pub lines: Vec<String>,
    pub ms_per_audio_second: u64,
    pub gemini: GeminiOptions,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            lines: vec![
                "Today we look at how attention works inside a transformer.".into(),
                "Each token builds a query, a key and a value.".into(),
                "We then train the whole network with gradient descent.".into(),
            ],
            ms_per_audio_second: 100,
            gemini: GeminiOptions::default(),
        }
    }
}

/// One installed copy of the app: its own data folder, fakes and host process.
pub struct App {
    pub dir: PathBuf,
    pub gemini: FakeGemini,
    pub media: MediaServer,
    opts: Options,
    youtube: HashMap<String, String>,
    host: Option<Host>,
    transcript: Arc<Mutex<Vec<String>>>,
}

impl App {
    pub fn new(opts: Options) -> App {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("vto-contract-{}-{nanos}", std::process::id()));
        fs::create_dir_all(dir.join("data")).unwrap();
        App {
            dir,
            gemini: FakeGemini::start(opts.gemini),
            media: MediaServer::start(),
            opts,
            youtube: HashMap::new(),
            host: None,
            transcript: Arc::default(),
        }
    }

    pub fn data_dir(&self) -> PathBuf {
        self.dir.join("data")
    }
    fn keystore_path(&self) -> PathBuf {
        self.dir.join("keystore.bin")
    }
    fn notifications_path(&self) -> PathBuf {
        self.dir.join("notifications.jsonl")
    }
    fn stderr_path(&self) -> PathBuf {
        self.dir.join("host-stderr.log")
    }

    /// Lets the fake YouTube extractor find audio for `page_url` at `media_path`.
    pub fn youtube(&mut self, page_url: &str, media_path: &str) {
        self.youtube.insert(page_url.into(), self.media.url(media_path));
    }

    pub fn launch(&mut self) {
        let config = json!({
            "data_dir": self.data_dir(),
            "keystore_path": self.keystore_path(),
            "notifications_path": self.notifications_path(),
            "gemini_base_url": self.gemini.base_url(),
            "allow_loopback_media": true,
            "youtube_audio": self.youtube,
            "transcriber": {"lines": self.opts.lines, "ms_per_audio_second": self.opts.ms_per_audio_second},
        });
        let path = self.dir.join("host-config.json");
        fs::write(&path, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
        self.host = Some(Host::spawn(&path, &self.stderr_path(), self.transcript.clone()));
    }

    /// Kills the process outright, as the OS does, and starts it again.
    pub fn restart(&mut self) {
        if let Some(mut h) = self.host.take() {
            h.kill();
        }
        self.launch();
    }

    fn host(&mut self) -> &mut Host {
        self.host.as_mut().expect("launch() the app first")
    }

    pub fn call(&mut self, origin: &str, method: &str, path: &str, body: Value) -> Resp {
        let req = json!({"op": "call", "method": method, "path": path, "body": body, "origin": origin});
        let r = self.host().request(req.clone());
        let status = r["status"].as_u64().unwrap_or_else(|| panic!("no status in {r} for {req}")) as u16;
        Resp { status, body: r["body"].clone() }
    }

    pub fn app_call(&mut self, method: &str, path: &str, body: Value) -> Resp {
        self.call(APP, method, path, body)
    }

    pub fn set_key(&mut self, key: &str) -> Resp {
        let r = self.app_call("PUT", "/settings/api-key", json!({ "api_key": key }));
        assert_eq!(r.status, 200, "PUT /settings/api-key: {r:?}");
        r
    }

    pub fn settings(&mut self) -> Resp {
        let r = self.app_call("GET", "/settings", Value::Null);
        assert_eq!(r.status, 200, "GET /settings: {r:?}");
        r
    }

    pub fn open(&mut self, url: &str) -> Resp {
        self.app_call("POST", "/browser/open", json!({ "url": url }))
    }

    /// POST /jobs from the overlay on `page_url`.
    pub fn create_job(&mut self, page_url: &str, media_url: Option<&str>, title: &str, duration_ms: u64) -> Resp {
        self.call(
            page_url,
            "POST",
            "/jobs",
            json!({"page_url": page_url, "media_url": media_url, "title": title, "duration_ms": duration_ms}),
        )
    }

    /// GET /jobs from the overlay on `page_url`.
    pub fn job(&mut self, page_url: &str) -> Resp {
        self.call(page_url, "GET", "/jobs", json!({ "page_url": page_url }))
    }

    pub fn subtitles(&mut self, page_url: &str, video_key: &str) -> Resp {
        self.call(page_url, "GET", &format!("/videos/{video_key}/subtitles"), Value::Null)
    }

    /// Polls GET /jobs every `every_ms` until `done` holds; returns every body seen.
    pub fn poll_job(&mut self, page_url: &str, every_ms: u64, timeout_s: u64, done: impl Fn(&Value) -> bool) -> Vec<Value> {
        let end = Instant::now() + Duration::from_secs(timeout_s);
        let mut seen = Vec::new();
        loop {
            let r = self.job(page_url);
            assert_eq!(r.status, 200, "GET /jobs while polling: {r:?}");
            let finished = done(&r.body);
            seen.push(r.body);
            if finished {
                return seen;
            }
            assert!(Instant::now() < end, "timed out; stages seen: {:?}; last: {}", stages(&seen), seen.last().unwrap());
            sleep_ms(every_ms);
        }
    }

    /// Polls until the job is `ready`; a `failed` job fails the test.
    pub fn wait_ready(&mut self, page_url: &str) -> Vec<Value> {
        self.poll_job(page_url, 20, 180, |b| {
            assert_ne!(b["stage"], "failed", "job failed: {b}");
            b["stage"] == "ready"
        })
    }

    /// Sets the key, creates the job, and waits for `ready`. Returns the
    /// POST /jobs body followed by every GET /jobs body seen.
    pub fn run_job(&mut self, page_url: &str, media_url: Option<&str>, title: &str, duration_ms: u64) -> Vec<Value> {
        let r = self.create_job(page_url, media_url, title, duration_ms);
        assert_eq!(r.status, 200, "POST /jobs: {r:?}");
        let mut seen = vec![r.body];
        seen.extend(self.wait_ready(page_url));
        seen
    }

    pub fn notifications(&self) -> Vec<Value> {
        fs::read_to_string(self.notifications_path())
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("notification {l:?}: {e}")))
            .collect()
    }

    pub fn wait_notification(&self, kind: &str, job_id: &str) -> Value {
        let end = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(n) = self.notifications().into_iter().find(|n| n["kind"] == kind && n["job_id"] == job_id) {
                return n;
            }
            assert!(Instant::now() < end, "no {kind} notification for {job_id}; shade: {:?}", self.notifications());
            sleep_ms(50);
        }
    }

    pub fn webview(&mut self) -> Value {
        self.host().request(json!({"op": "webview"}))
    }

    pub fn page_loaded(&mut self, url: &str) {
        let r = self.host().request(json!({"op": "page_loaded", "url": url}));
        assert_eq!(r["ok"], true, "page_loaded: {r}");
    }

    pub fn set_cookie(&mut self, url: &str, name: &str, value: &str) {
        let r = self.host().request(json!({"op": "set_cookie", "url": url, "name": name, "value": value}));
        assert_eq!(r["ok"], true, "set_cookie: {r}");
    }

    pub fn commands(&mut self) -> Vec<String> {
        let r = self.host().request(json!({"op": "commands"}));
        r["commands"]
            .as_array()
            .unwrap_or_else(|| panic!("commands: {r}"))
            .iter()
            .map(|c| c.as_str().unwrap_or_default().to_string())
            .collect()
    }

    pub fn block_network(&mut self) {
        let r = self.host().request(json!({"op": "network", "blocked": true}));
        assert_eq!(r["ok"], true, "network: {r}");
    }

    pub fn pricing(&mut self) -> (f64, f64, f64) {
        let r = self.host().request(json!({"op": "pricing"}));
        let f = |k: &str| r[k].as_f64().unwrap_or_else(|| panic!("pricing.{k}: {r}"));
        (f("input_usd_per_million"), f("output_usd_per_million"), f("krw_per_usd"))
    }

    /// Every file under the data folder whose bytes contain `needle`.
    pub fn data_files_containing(&self, needle: &[u8]) -> Vec<PathBuf> {
        files_under(&self.data_dir()).into_iter().filter(|p| contains(&read(p), needle)).collect()
    }

    /// SQLite databases under the data folder, with their -wal/-journal files.
    pub fn sqlite_files(&self) -> Vec<PathBuf> {
        let all = files_under(&self.data_dir());
        let dbs: Vec<PathBuf> = all.iter().filter(|p| read(p).starts_with(b"SQLite format 3\0")).cloned().collect();
        all.into_iter()
            .filter(|p| {
                dbs.iter().any(|db| {
                    let (d, f) = (db.to_string_lossy(), p.to_string_lossy());
                    f == d || f == format!("{d}-wal") || f == format!("{d}-journal")
                })
            })
            .collect()
    }

    pub fn sqlite_contains(&self, needle: &[u8]) -> bool {
        self.sqlite_files().iter().any(|p| contains(&read(p), needle))
    }

    pub fn keystore_bytes(&self) -> Vec<u8> {
        fs::read(self.keystore_path()).unwrap_or_default()
    }

    /// What the app logged (stderr) and answered (stdout) so far.
    pub fn logs(&self) -> Vec<u8> {
        let mut out = fs::read(self.stderr_path()).unwrap_or_default();
        out.extend(self.transcript.lock().unwrap().join("\n").into_bytes());
        out
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.host.take();
        self.media.stop();
        self.gemini.stop();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

pub fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}

fn read(p: &Path) -> Vec<u8> {
    fs::read(p).unwrap_or_default()
}

pub fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

pub fn contains_ci(hay: &[u8], needle: &str) -> bool {
    contains(&hay.to_ascii_lowercase(), needle.to_ascii_lowercase().as_bytes())
}

/// Stage names in the order seen, consecutive repeats collapsed.
pub fn stages(seen: &[Value]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in seen {
        let s = b["stage"].as_str().unwrap_or("?").to_string();
        if out.last() != Some(&s) {
            out.push(s);
        }
    }
    out
}

/// `(stage rank, percent)`: how far a job is; compares across stages.
pub fn progress(body: &Value) -> (usize, i64) {
    let stage = body["stage"].as_str().unwrap_or("?");
    let rank = STAGES.iter().position(|s| *s == stage).unwrap_or_else(|| panic!("unexpected stage in {body}"));
    (rank, body["percent"].as_i64().unwrap_or_else(|| panic!("percent missing in {body}")))
}

pub fn cues(subtitles: &Resp) -> Vec<Value> {
    assert_eq!(subtitles.status, 200, "GET /videos/{{video_key}}/subtitles: {subtitles:?}");
    subtitles.body["cues"].as_array().unwrap_or_else(|| panic!("cues missing: {}", subtitles.body)).clone()
}

pub fn ms(cue: &Value, field: &str) -> i64 {
    cue[field].as_i64().unwrap_or_else(|| panic!("{field} missing in {cue}"))
}

pub fn has_hangul(s: &str) -> bool {
    s.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c))
}
