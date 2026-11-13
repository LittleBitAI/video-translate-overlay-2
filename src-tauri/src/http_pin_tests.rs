//! A pinned request goes to its address and nowhere else, on every OS: the
//! name is never resolved again (these names cannot resolve at all), yet it is
//! what the server sees in `Host` and, for https, in the TLS server name.

use super::*;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

const NAME: &str = "media.vto-test.invalid";

#[test]
fn plain_http_goes_to_the_pinned_address_with_its_name() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let at = server.local_addr().unwrap();
    let seen = std::thread::spawn(move || {
        let (s, _) = server.accept().unwrap();
        let mut head = Vec::new();
        let mut r = BufReader::new(s.try_clone().unwrap());
        loop {
            let mut l = String::new();
            if r.read_line(&mut l).unwrap() == 0 || l.trim().is_empty() {
                break;
            }
            head.push(l.trim().to_string());
        }
        (&s).write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").unwrap();
        head
    });
    let url = format!("http://{NAME}:{}/v/a.m4a", at.port());
    let mut req = Request::get(&url);
    req.connect_to = Some(at);
    let r = send(req).unwrap();
    assert_eq!((r.status, r.remote_ip), (200, Some(at.ip())));
    let head = seen.join().unwrap();
    assert!(head[0].starts_with("GET /v/a.m4a "), "{head:?}");
    assert!(head.contains(&format!("Host: {NAME}:{}", at.port())), "{head:?}");
}

#[test]
fn https_goes_to_the_pinned_address_and_asks_for_its_name() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let at = server.local_addr().unwrap();
    let hello = std::thread::spawn(move || {
        let (mut s, _) = server.accept().unwrap();
        let mut buf = vec![0u8; 4096];
        let n = s.read(&mut buf).unwrap();
        buf.truncate(n);
        buf // dropped without answering: no certificate, so no request is ever sent
    });
    let url = format!("https://{NAME}:{}/a.m4a", at.port());
    let mut req = Request::get(&url);
    req.connect_to = Some(at);
    req.timeout_ms = 5_000;
    assert!(send(req).is_err(), "a server that cannot prove the name gets no request");
    let hello = hello.join().unwrap();
    assert_eq!(hello.first(), Some(&0x16), "a TLS handshake reached the pinned address");
    assert!(hello.windows(NAME.len()).any(|w| w == NAME.as_bytes()), "the server name in the ClientHello is the URL's");
}
