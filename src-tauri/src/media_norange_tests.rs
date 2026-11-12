//! A server that ignores Range (answers 200 with the whole file) still gets an
//! MP4's `moov` checked for DRM; one it cannot be checked in is refused.

use super::*;
use std::io::{BufRead, BufReader};

/// Serves `files` whole on every request, Range or not, until the process ends.
fn serve_whole(files: Vec<(&'static str, Vec<u8>)>) -> String {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in server.incoming() {
            let Ok(s) = s else { continue };
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut path = String::new();
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                    break;
                }
                if let Some(p) = l.strip_prefix("GET ") {
                    path = p.split(' ').next().unwrap_or("").to_string();
                }
            }
            let body = files.iter().find(|(p, _)| *p == path).map(|(_, b)| b.as_slice()).unwrap_or(&[]);
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = (&s).write_all(head.as_bytes());
            let _ = (&s).write_all(body); // a client that stops reading early just breaks the pipe
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    b.extend_from_slice(kind);
    b.extend_from_slice(body);
    b
}

#[test]
fn moov_after_a_large_mdat_is_checked_without_range_support() {
    let ftyp = mp4_box(b"ftyp", b"isom\0\0\0\0isomiso6");
    let protected = mp4_box(b"moov", &mp4_box(b"trak", &mp4_box(b"enca", &mp4_box(b"sinf", &mp4_box(b"tenc", &[0; 24])))));
    let clear = mp4_box(b"moov", &mp4_box(b"trak", &mp4_box(b"mp4a", &[0; 28])));
    // Past the probe's 64 KiB head, so `moov` is only seen by reading on.
    let mdat = mp4_box(b"mdat", &vec![0; 300_000]);
    // Past the 64 MiB the probe will skip.
    let huge = mp4_box(b"mdat", &vec![0; (64 << 20) + 1024]);
    let base = serve_whole(vec![
        ("/late.mp4", [&ftyp[..], &mdat, &protected].concat()),
        ("/clear.mp4", [&ftyp[..], &mdat, &clear].concat()),
        ("/huge.mp4", [&ftyp[..], &huge, &clear].concat()),
    ]);
    let probe_at = |path: &str| probe(&Url::parse(&format!("{base}{path}")).unwrap(), true);
    assert_eq!(probe_at("/late.mp4").unwrap_err().code, "drm");
    assert!(matches!(probe_at("/clear.mp4"), Ok(Source::File { ext: "m4a", .. })));
    assert_eq!(probe_at("/huge.mp4").unwrap_err().code, "unverifiable", "an unchecked file is not let through");
}
