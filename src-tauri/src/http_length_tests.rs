//! A body that ends before its Content-Length is an error, through WinHTTP
//! and through the portable client alike, so a dropped connection never
//! passes as a complete download (an HLS segment included).

use super::*;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

/// Serves `n` connections; each declares 100 bytes and sends 40 before closing.
fn short_server(n: usize) -> std::net::SocketAddr {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let at = server.local_addr().unwrap();
    std::thread::spawn(move || {
        for _ in 0..n {
            let Ok((s, _)) = server.accept() else { return };
            let mut r = BufReader::new(s.try_clone().unwrap());
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                    break;
                }
            }
            let _ = (&s).write_all(b"HTTP/1.1 200 OK\r\nContent-Type: audio/aac\r\nContent-Length: 100\r\nConnection: close\r\n\r\n");
            let _ = (&s).write_all(&[0xAB; 40]);
        } // the stream drops here: the connection closes 60 bytes early
    });
    at
}

#[test]
fn a_body_cut_short_of_its_length_is_an_error() {
    let at = short_server(2);
    let url = format!("http://{at}/seg0.aac");
    // Unpinned: WinHTTP on Windows (which may report the drop itself, or leave it to the length check).
    assert!(send(Request::get(&url)).unwrap().bytes(1 << 20).is_err());
    // Pinned: the portable client, on every OS.
    let mut req = Request::get(&url);
    req.connect_to = Some(at);
    let e = send(req).unwrap().bytes(1 << 20).unwrap_err();
    assert!(e.0.contains("60 bytes short"), "{e}");
}

#[test]
fn a_cut_hls_segment_fails_the_download() {
    let at = short_server(1);
    let parts = vec![crate::media::Part { url: format!("http://{at}/seg0.aac"), range: None }];
    let dest = std::env::temp_dir().join(format!("vto-cut-{}.aac", std::process::id()));
    let r = crate::media::download(&crate::media::Source::Hls { parts, ext: "aac" }, &dest, true, &mut |_| {});
    let _ = std::fs::remove_file(&dest);
    assert!(r.is_err(), "a segment that ended early passed as complete");
}
