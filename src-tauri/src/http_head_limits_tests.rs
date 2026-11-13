//! A server cannot make the portable client (every media request, on every
//! OS) hold an endless response head: a header line past 8 KiB, or a head
//! past 64 KiB in all, ends the request with an error instead.

use super::*;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// Answers one request with `start`, then `more` over and over until the client goes away.
fn endless(start: &'static [u8], more: &'static [u8]) -> std::net::SocketAddr {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let at = server.local_addr().unwrap();
    std::thread::spawn(move || {
        let Ok((s, _)) = server.accept() else { return };
        let mut r = BufReader::new(s.try_clone().unwrap());
        loop {
            let mut l = String::new();
            if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                break;
            }
        }
        if (&s).write_all(start).is_err() {
            return;
        }
        // At most 1 GiB, should the client never stop reading.
        for _ in 0..(1 << 30) / more.len() {
            if (&s).write_all(more).is_err() {
                return;
            }
        }
    });
    at
}

fn pinned(at: std::net::SocketAddr) -> Result<Response, HttpError> {
    let url = format!("http://{at}/talk.mp4");
    let mut req = Request::get(&url);
    req.connect_to = Some(at);
    send(req)
}

#[test]
fn a_header_line_with_no_end_is_refused() {
    let started = Instant::now();
    let e = pinned(endless(b"HTTP/1.1 200 OK\r\nX: ", &[b'a'; 4096])).err().expect("an endless header line was taken");
    assert!(e.0.contains("line too long"), "{e}");
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn a_head_of_endless_short_headers_is_refused() {
    let e = pinned(endless(b"HTTP/1.1 200 OK\r\n", b"X-Pad: 0123456789\r\n")).err().expect("an endless head was taken");
    assert!(e.0.contains("head too large"), "{e}");
}

#[test]
fn an_endless_status_line_is_refused() {
    let e = pinned(endless(b"HTTP/1.1 200 ", &[b'O'; 4096])).err().expect("an endless status line was taken");
    assert!(e.0.contains("line too long"), "{e}");
}

#[test]
fn endless_chunked_trailers_are_refused() {
    let at = endless(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\n", b"X-Trailer: 1\r\n");
    let e = pinned(at).unwrap().bytes(1 << 20).unwrap_err();
    assert!(e.0.contains("too many trailer fields"), "{e}");
}
