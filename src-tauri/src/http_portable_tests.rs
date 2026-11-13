//! The HTTP client Android uses, run here against a loopback server and,
//! opt-in, against a real HTTPS host.

use super::*;
use std::io::{BufRead, BufReader, Write};

/// Answers each connection with the next canned response, recording request heads.
fn serve(answers: Vec<&'static str>) -> (String, std::thread::JoinHandle<Vec<Vec<String>>>) {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", server.local_addr().unwrap().port());
    let seen = std::thread::spawn(move || {
        let mut heads = Vec::new();
        for answer in answers {
            let (s, _) = server.accept().unwrap();
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut head = Vec::new();
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap() == 0 || l.trim().is_empty() {
                    break;
                }
                head.push(l.trim().to_string());
            }
            (&s).write_all(answer.as_bytes()).unwrap();
            heads.push(head);
        }
        heads
    });
    (base, seen)
}

fn send(req: Request<'_>) -> Result<Response, HttpError> {
    portable::send(&url::Url::parse(req.url).unwrap(), &req)
}

#[test]
fn redirects_are_followed_only_when_asked_and_chunks_are_joined() {
    let (base, seen) = serve(vec![
        "HTTP/1.1 302 Found\r\nLocation: /b\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: text/plain\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n",
        "HTTP/1.1 301 Moved\r\nLocation: https://elsewhere.example/\r\nContent-Length: 0\r\n\r\n",
    ]);
    let url = format!("{base}/a");
    let r = send(Request::get(&url)).unwrap();
    assert_eq!((r.status, r.content_type.clone()), (200, Some("text/plain".into())));
    assert_eq!(r.bytes(100).unwrap(), b"abcde");
    let mut manual = Request::get(&url);
    manual.follow_redirects = false;
    let r = send(manual).unwrap();
    assert_eq!((r.status, r.location.as_deref()), (301, Some("https://elsewhere.example/")));
    let heads = seen.join().unwrap();
    assert!(heads[1][0].starts_with("GET /b "), "{heads:?}");
    let port = base.rsplit(':').next().unwrap();
    assert!(heads[0].contains(&format!("Host: 127.0.0.1:{port}")), "{heads:?}");
}

/// Something that is not TLS on the other end is an error, not a hang or a panic.
#[test]
fn a_server_that_does_not_speak_tls_fails_the_request() {
    let (base, _seen) = serve(vec!["HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"]);
    let url = base.replace("http://", "https://") + "/";
    let mut req = Request::get(&url);
    req.timeout_ms = 5_000;
    assert!(send(req).is_err());
}

/// Real TLS with certificate checks: the host's name verifies, its bare address does not.
/// `cargo test -p video-translate-overlay --lib portable_tests -- --ignored` (needs github.com).
#[test]
#[ignore = "needs the network"]
fn https_verifies_the_certificate_name() {
    let r = send(Request::get("https://github.com/robots.txt")).unwrap();
    assert_eq!(r.status, 200);
    assert!(r.remote_ip.is_some());
    assert!(String::from_utf8(r.bytes(4096).unwrap()).unwrap().contains("User-agent"));
    let ip = std::net::ToSocketAddrs::to_socket_addrs(&("github.com", 443)).unwrap().next().unwrap().ip();
    let bare = match ip {
        std::net::IpAddr::V4(v) => format!("https://{v}/"),
        std::net::IpAddr::V6(v) => format!("https://[{v}]/"),
    };
    let e = send(Request::get(&bare)).err().expect("a certificate without that address must fail");
    assert!(e.0.to_ascii_lowercase().contains("certificate"), "{}", e.0);
}
