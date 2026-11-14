//! The app's screens and API over HTTP on one port (`vto-web`): the built
//! screens (`/`, `/preferences`) and the API of `docs/design/api.yaml`, the
//! same commands the Tauri window reaches through IPC.
//!
//! Only this device's own pages may use it: a request must name this server
//! as `127.0.0.1`, `localhost` or `[::1]` in `Host` (no DNS rebinding), and a
//! request a browser sends from another origin (its `Origin`) is refused, so no
//! web page can read, replace or spend the key.

use crate::api::{self, err, ApiResult};
use crate::Core;
use serde_json::{json, Map, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The screens: each opens the built `index.html`, which shows the one its path names.
pub const SCREENS: [&str; 3] = ["/", "/preferences", "/library"];

const MAX_HEAD: u64 = 64 << 10;
const MAX_BODY: usize = 1 << 20;

/// Serves until the listener fails.
pub fn serve(listener: TcpListener, core: Arc<Core>, dist: PathBuf) {
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let dist = Arc::new(dist);
    for s in listener.incoming() {
        let Ok(s) = s else { continue };
        let (core, dist) = (core.clone(), dist.clone());
        std::thread::spawn(move || {
            if let Err(e) = handle(s, &core, &dist, port) {
                crate::log(format!("web: {e}"));
            }
        });
    }
}

/// One request: its method, path, query and the headers that matter.
struct Req {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    host: String,
    origin: Option<String>,
    body: Vec<u8>,
}

fn read_req(s: &TcpStream) -> std::io::Result<Option<Req>> {
    let mut r = BufReader::new(s.take(MAX_HEAD + MAX_BODY as u64));
    let mut line = String::new();
    if r.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let mut parts = line.split_whitespace();
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else { return Ok(None) };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut req = Req {
        method: method.to_string(),
        path: path.to_string(),
        query: url::form_urlencoded::parse(query.as_bytes()).into_owned().collect(),
        host: String::new(),
        origin: None,
        body: Vec::new(),
    };
    let (mut len, mut head) = (0usize, 0u64);
    loop {
        let mut h = String::new();
        let n = r.read_line(&mut h)?;
        head += n as u64;
        if n == 0 || h.trim().is_empty() || head > MAX_HEAD {
            break;
        }
        let Some((k, v)) = h.split_once(':') else { continue };
        let v = v.trim();
        match k.trim().to_ascii_lowercase().as_str() {
            "host" => req.host = v.to_ascii_lowercase(),
            "origin" => req.origin = Some(v.to_ascii_lowercase()),
            "content-length" => len = v.parse().unwrap_or(usize::MAX),
            _ => {}
        }
    }
    if len > MAX_BODY {
        return Ok(None);
    }
    req.body = vec![0; len];
    r.read_exact(&mut req.body)?;
    Ok(Some(req))
}

/// `Host` names this server on this device, and a browser's `Origin`, when it sends one, is this server.
fn trusted(req: &Req, port: u16) -> bool {
    let local = ["127.0.0.1", "localhost", "[::1]"].iter().any(|h| req.host == format!("{h}:{port}"));
    local && req.origin.as_ref().is_none_or(|o| *o == format!("http://{}", req.host))
}

fn handle(mut s: TcpStream, core: &Core, dist: &Path, port: u16) -> std::io::Result<()> {
    let Some(req) = read_req(&s)? else {
        return send(&mut s, 400, "text/plain; charset=utf-8", b"bad request");
    };
    if !trusted(&req, port) {
        return send(&mut s, 403, "text/plain; charset=utf-8", b"forbidden");
    }
    if req.method == "GET" && req.path == "/build" {
        return json_reply(&mut s, Ok(json!({"head": env!("VTO_BUILD_HEAD"), "pid": std::process::id()})));
    }
    let mut args = match serde_json::from_slice::<Value>(&req.body) {
        Ok(Value::Object(m)) => m,
        _ if req.body.iter().all(u8::is_ascii_whitespace) => Map::new(),
        _ => return json_reply(&mut s, Err(err(400, "bad_json", "요청 본문이 JSON이 아닙니다."))),
    };
    // GET has no body: its arguments (P7's `page_url`) come in the query.
    for (k, v) in &req.query {
        args.entry(k.clone()).or_insert_with(|| json!(v));
    }
    if let Some((cmd, args)) = crate::commands::route(&req.method, &req.path, &Value::Object(args)) {
        return json_reply(&mut s, call(core, cmd, &args));
    }
    if req.method != "GET" && req.method != "HEAD" {
        return json_reply(&mut s, Err(err(404, "no_route", format!("{} {}", req.method, req.path))));
    }
    let file = if SCREENS.contains(&req.path.as_str()) { Some(dist.join("index.html")) } else { asset(dist, &req.path) };
    match file.and_then(|f| std::fs::read(&f).ok().map(|b| (f, b))) {
        Some((f, b)) => send(&mut s, 200, content_type(&f), &b),
        None => send(&mut s, 404, "text/plain; charset=utf-8", "찾을 수 없는 화면입니다.".as_bytes()),
    }
}

/// A built file under `dist`, by a path of plain segments only.
fn asset(dist: &Path, path: &str) -> Option<PathBuf> {
    let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let plain = |s: &&str| !s.is_empty() && !s.starts_with('.') && !s.contains(['\\', ':']);
    segs.iter().all(plain).then(|| segs.iter().fold(dist.to_path_buf(), |p, s| p.join(s))).filter(|p| p.is_file())
}

fn content_type(f: &Path) -> &'static str {
    match f.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "json" => "application/json",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Runs a command of [`crate::commands::COMMANDS`] with its arguments.
pub fn call(core: &Core, cmd: &str, a: &Value) -> ApiResult {
    let s = |k: &str| a[k].as_str().unwrap_or_default().to_string();
    match cmd {
        "check_updates" => core.updates.check(&core.cfg.data_dir, a["manual"].as_bool().unwrap_or(false)),
        "dismiss_update" => core.updates.dismiss(&core.cfg.data_dir, &s("version")),
        "open_update" => crate::updates::open(core, &s("version")),
        "browser_open" => api::browser_open(core, &s("url")),
        "get_browser" => api::get_browser(core),
        "get_library" => api::get_library(core),
        "library_action" => api::library_action(core, &a["action"]),
        "clear_site_data" => api::clear_site_data(core),
        "get_settings" => api::get_settings(core),
        "put_api_key" => api::put_api_key(core, &s("api_key")),
        "delete_api_key" => api::delete_api_key(core),
        "create_job" if a["retranslate"] == true => api::retranslate_job(core, &s("page_url")),
        "create_job" => api::create_job_with_consent(core, &s("page_url"), a["media_url"].as_str(), a["title"].as_str().unwrap_or_default(), a["duration_ms"].as_i64().unwrap_or(0), a["deletion_token"].as_str()),
        "get_job" => api::get_job(core, &s("page_url")),
        "get_subtitles" => api::get_subtitles(core, &s("video_key")),
        other => Err(err(404, "no_route", other.to_string())),
    }
}

fn json_reply(s: &mut TcpStream, r: ApiResult) -> std::io::Result<()> {
    let (status, body) = match r {
        Ok(v) => (200, v),
        Err(e) => (e.status, json!(e)),
    };
    send(s, status, "application/json", body.to_string().as_bytes())
}

fn send(s: &mut TcpStream, status: u16, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes())?;
    s.write_all(body)?;
    s.flush()
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
