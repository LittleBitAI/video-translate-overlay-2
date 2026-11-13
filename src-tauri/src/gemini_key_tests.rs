//! A key Gemini stops taking mid-job (HTTP 400 with an `API_KEY_INVALID`
//! reason, as Google answers an invalid or expired key) is a refused key,
//! which ends the job as `key_rejected`, not a network trouble to retry.

use super::*;
use std::io::{BufRead, BufReader, Read, Write};

/// Answers every request with `status` and `body` until the process ends.
fn serve(status: u16, body: &'static str) -> String {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let at = server.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in server.incoming() {
            let Ok(s) = s else { continue };
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut len = 0usize;
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                    break;
                }
                if let Some(v) = l.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
            }
            let _ = (&mut r).take(len as u64).read_to_end(&mut Vec::new());
            let head = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = (&s).write_all(head.as_bytes());
            let _ = (&s).write_all(body.as_bytes());
        }
    });
    format!("http://{at}")
}

fn client(base_url: String) -> Client {
    Client { base_url, key: "AIzaSyD-test-key-0123".into(), model: MODEL.into() }
}

const INVALID: &str = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT",
  "details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"API_KEY_INVALID","domain":"googleapis.com"}]}}"#;

#[test]
fn a_400_that_names_the_key_is_a_refused_key() {
    let c = client(serve(400, INVALID));
    assert!(matches!(c.order(&json!({"task": "translate"})), Err(Error::Key(_))));
    assert!(matches!(c.check_key(), Err(Error::Key(_))));
}

#[test]
fn another_400_is_not_blamed_on_the_key() {
    let c = client(serve(400, r#"{"error":{"code":400,"message":"Request contains an invalid argument.","status":"INVALID_ARGUMENT"}}"#));
    assert!(matches!(c.order(&json!({"task": "translate"})), Err(Error::Transient(_))));
}
