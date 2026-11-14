//! `vto-web`: the screens and the API on one port. Each screen route opens the
//! built `index.html`, built files are served by plain paths only, the API
//! answers as the Tauri commands do (GET's arguments in the query), P9 gives
//! back what P1 last opened, and only this device's own pages are served.

use super::*;
use crate::{AudioExtractor, Browser, Config, KeyStore, Notifier, Services, Transcriber};
use std::sync::Mutex;

struct Mem(Mutex<Option<String>>);

impl KeyStore for Mem {
    fn get(&self) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn set(&self, key: &str) -> Result<(), String> {
        *self.0.lock().unwrap() = Some(key.into());
        Ok(())
    }
    fn delete(&self) -> Result<(), String> {
        *self.0.lock().unwrap() = None;
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
impl Notifier for Nothing {
    fn ongoing(&self, _: &str, _: &str, _: &str, _: i64) {}
    fn ready(&self, _: &str, _: &str, _: &str) {}
}
impl Transcriber for Nothing {
    fn transcribe(&self, _: &[f32], _: &mut dyn FnMut(f32)) -> Result<Vec<crate::Segment>, String> {
        Ok(Vec::new())
    }
}
impl AudioExtractor for Nothing {
    fn audio_url(&self, _: &str) -> Result<String, crate::media::Refusal> {
        Err(crate::media::refuse("youtube_unavailable", "x"))
    }
}

/// A server on a free port with a fresh data folder; its port.
fn start(name: &str) -> u16 {
    start_with_core(name).0
}

fn start_with_core(name: &str) -> (u16, Arc<Core>) {
    let dir = std::env::temp_dir().join(format!("vto-web-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let dist = dir.join("dist");
    std::fs::create_dir_all(dist.join("assets")).unwrap();
    std::fs::write(dist.join("index.html"), "<!doctype html><title>screens</title>").unwrap();
    std::fs::write(dist.join("assets/app.js"), "export {}").unwrap();
    let svc = Services {
        keystore: Box::new(Mem(Mutex::new(None))),
        browser: Box::new(Nothing),
        notifier: Box::new(Nothing),
        transcriber: Box::new(Nothing),
        extractor: Box::new(Nothing),
    };
    let core = Core::start(Config { data_dir: dir.join("data"), gemini_base_url: "http://127.0.0.1:9".into(), allow_loopback_media: false }, svc).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let served = core.clone();
    std::thread::spawn(move || serve(listener, served, dist));
    (port, core)
}

/// `(status, content type, body)` of one request; `extra` is more header lines.
fn ask(port: u16, method: &str, target: &str, body: Option<Value>, extra: &str) -> (u16, String, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let host = if extra.to_ascii_lowercase().contains("host:") { String::new() } else { format!("Host: 127.0.0.1:{port}\r\n") };
    write!(s, "{method} {target} HTTP/1.1\r\n{host}{extra}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    let ctype = head.lines().find_map(|l| l.strip_prefix("Content-Type: ")).unwrap_or("").to_string();
    (status, ctype, body.to_string())
}

fn json(port: u16, method: &str, target: &str, body: Option<Value>) -> (u16, Value) {
    let (status, _, b) = ask(port, method, target, body, "");
    (status, serde_json::from_str(&b).unwrap_or(Value::Null))
}

#[test]
fn each_screen_route_opens_the_built_screens_and_files_are_served_by_plain_paths() {
    let port = start("screens");
    assert_eq!(json(port, "GET", "/build", None), (200, json!({"head": env!("VTO_BUILD_HEAD"), "pid": std::process::id()})));
    for screen in SCREENS {
        let (status, ctype, body) = ask(port, "GET", screen, None, "");
        assert_eq!((status, ctype.as_str()), (200, "text/html; charset=utf-8"), "{screen}");
        assert!(body.contains("<title>screens</title>"), "{screen}: {body}");
    }
    let (status, ctype, _) = ask(port, "GET", "/assets/app.js", None, "");
    assert_eq!((status, ctype.as_str()), (200, "text/javascript; charset=utf-8"));
    for missing in ["/nope", "/assets/../index.html", "/assets/%2e%2e/index.html", "/../Cargo.toml", "/preferences/x"] {
        assert_eq!(ask(port, "GET", missing, None, "").0, 404, "{missing}");
    }
}

#[test]
fn the_api_answers_as_the_commands_do_and_p9_gives_back_what_p1_opened() {
    let port = start("api");
    assert_eq!(json(port, "GET", "/browser", None), (200, json!({"url": null, "job": null})));
    let talk = "https://talks.example.org/attention";
    let (status, opened) = json(port, "POST", "/browser/open", Some(json!({"url": talk})));
    assert_eq!((status, &opened["opened_url"]), (200, &json!(talk)), "{opened}");
    assert_eq!(json(port, "GET", "/browser", None), (200, json!({"url": talk, "job": null})), "a page with no job yet");

    // The first YouTube open is Google's account screen, and P9 says so.
    let yt = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    let gate = json(port, "POST", "/browser/open", Some(json!({"url": yt}))).1;
    assert_eq!(gate["account_gate"], true, "{gate}");
    assert_eq!(json(port, "GET", "/browser", None).1["url"], gate["opened_url"]);

    let (status, bad) = json(port, "POST", "/browser/open", Some(json!({"url": "javascript:alert(1)"})));
    assert_eq!((status, &bad["error"]), (400, &json!("bad_url")), "{bad}");
    assert_eq!(json(port, "GET", "/settings", None).1["key_set"], false);
    // P7 takes its page_url from the query.
    let (status, none) = json(port, "GET", &format!("/jobs?page_url={}", url::form_urlencoded::byte_serialize(talk.as_bytes()).collect::<String>()), None);
    assert_eq!((status, &none["error"]), (404, &json!("no_job")), "{none}");
    assert_eq!(json(port, "GET", "/jobs", None).0, 400, "no page_url");
    assert_eq!(json(port, "GET", "/videos/abc/subtitles", None).0, 404);
    assert_eq!(json(port, "PATCH", "/settings", None).0, 404);
}

#[test]
fn p9_carries_the_opened_pages_latest_job() {
    let (port, core) = start_with_core("job");
    let talk = "https://talks.example.org/lecture-7";
    assert_eq!(json(port, "POST", "/browser/open", Some(json!({"url": talk}))).0, 200);
    let key = api::video_key(&url::Url::parse(talk).unwrap());
    core.store.create_job("job-1", &key, talk, "other", "Lecture 7", 60_000, None, crate::gemini::MODEL).unwrap();
    let (status, b) = json(port, "GET", "/browser", None);
    assert_eq!((status, &b["url"]), (200, &json!(talk)), "{b}");
    assert_eq!((&b["job"]["job_id"], &b["job"]["video_key"], &b["job"]["title"]), (&json!("job-1"), &json!(key), &json!("Lecture 7")), "{b}");
}

#[test]
fn only_this_devices_own_pages_are_served() {
    let port = start("origin");
    assert_eq!(ask(port, "GET", "/build", None, "Origin: https://outside.example\r\n").0, 403);
    let open = Some(json!({"url": "https://talks.example.org/a"}));
    let own = format!("Origin: http://127.0.0.1:{port}\r\n");
    assert_eq!(ask(port, "POST", "/browser/open", open.clone(), &own).0, 200);
    let localhost = format!("Host: localhost:{port}\r\nOrigin: http://localhost:{port}\r\n");
    assert_eq!(ask(port, "POST", "/browser/open", open.clone(), &localhost).0, 200);
    // Another site's page, a page that is this port on another name, and a name rebound to this device.
    assert_eq!(ask(port, "POST", "/browser/open", open.clone(), "Origin: https://evil.example\r\n").0, 403);
    assert_eq!(ask(port, "DELETE", "/settings/api-key", None, "Origin: null\r\n").0, 403);
    assert_eq!(ask(port, "GET", "/settings", None, &format!("Host: evil.example:{port}\r\n")).0, 403);
    assert_eq!(ask(port, "GET", "/settings", None, "Host: 127.0.0.1:1\r\n").0, 403);
}
