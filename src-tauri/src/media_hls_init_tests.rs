//! An HLS playlist's init sections are checked as a file's `moov` is before a
//! job is made (P6's 422, R20): one with Common Encryption marks is refused as
//! `drm`, whichever section it starts. And each is read at its byte range, also
//! from a server that ignores Range and sends the whole file, so a playable
//! playlist is not refused for the bytes in front of its init section.
//!
//! The pieces are `tests/fixtures/hls_maps` (fMP4, sound on both init sections).

use super::tracks_tests::{fixtures, serve_dir};
use super::*;
use std::io::{BufRead, BufReader, Write};

fn read(name: &str) -> Vec<u8> {
    std::fs::read(fixtures("hls_maps").join(name)).unwrap()
}

/// `init` with its AAC sample entry marked encrypted (`mp4a` -> `enca`), as a
/// protected init section is.
fn encrypted(init: &[u8]) -> Vec<u8> {
    let mut b = init.to_vec();
    let at = b.windows(4).position(|w| w == b"mp4a").expect("no mp4a");
    b[at..at + 4].copy_from_slice(b"enca");
    b
}

/// Serves `files` with 200 and the whole body to every request, Range or not.
fn serve_ignoring_range(files: Vec<(String, Vec<u8>)>) -> String {
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
            let body = files.iter().find(|(p, _)| *p == path).map(|(_, b)| b.clone());
            let head = match &body {
                Some(b) => format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", b.len()),
                None => "HTTP/1.1 404 X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
            };
            let _ = (&s).write_all(head.as_bytes());
            if let Some(b) = body {
                let _ = (&s).write_all(&b);
            }
        }
    });
    format!("http://127.0.0.1:{port}")
}

#[test]
fn an_encrypted_init_section_is_refused_as_drm() {
    let one = "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-MAP:URI=\"enc.mp4\"\n#EXTINF:4.0,\na0.m4s\n#EXT-X-ENDLIST\n";
    let base = serve_dir("hls_maps", vec![("/enc.mp4".into(), encrypted(&read("init-a.mp4"))), ("/one.m3u8".into(), one.into())]);
    let e = probe(&Url::parse(&format!("{base}/one.m3u8")).unwrap(), true).unwrap_err();
    assert_eq!(e.code, "drm", "{}", e.message);
}

#[test]
fn an_encrypted_init_section_of_a_later_section_is_refused_too() {
    let playlist = std::fs::read_to_string(fixtures("hls_maps").join("index.m3u8")).unwrap().replace("init-b.mp4", "enc-b.mp4");
    let base = serve_dir("hls_maps", vec![("/enc-b.mp4".into(), encrypted(&read("init-b.mp4"))), ("/later.m3u8".into(), playlist.into_bytes())]);
    let e = probe(&Url::parse(&format!("{base}/later.m3u8")).unwrap(), true).unwrap_err();
    assert_eq!(e.code, "drm", "{}", e.message);
}

/// One file: 9 MiB with no track in it, then `init`, then `a0.m4s`, both by byte range.
fn packed(init: &[u8]) -> (Vec<u8>, String) {
    let (seg, pad) = (read("a0.m4s"), 9usize << 20);
    let mut file = vec![0u8; pad];
    file.extend_from_slice(init);
    file.extend_from_slice(&seg);
    let playlist = format!(
        "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-MAP:URI=\"all.mp4\",BYTERANGE=\"{}@{pad}\"\n#EXTINF:4.0,\n#EXT-X-BYTERANGE:{}@{}\nall.mp4\n#EXT-X-ENDLIST\n",
        init.len(),
        seg.len(),
        pad + init.len()
    );
    (file, playlist)
}

#[test]
fn an_init_section_at_an_offset_is_checked_there_when_range_is_ignored() {
    let (file, playlist) = packed(&read("init-a.mp4"));
    let base = serve_ignoring_range(vec![("/all.mp4".into(), file), ("/p.m3u8".into(), playlist.into_bytes())]);
    let src = probe(&Url::parse(&format!("{base}/p.m3u8")).unwrap(), true).expect("a playable playlist was refused");
    assert!(matches!(src, Source::Hls { .. }));

    // And its protection marks are read there too.
    let (file, playlist) = packed(&encrypted(&read("init-a.mp4")));
    let base = serve_ignoring_range(vec![("/all.mp4".into(), file), ("/p.m3u8".into(), playlist.into_bytes())]);
    let e = probe(&Url::parse(&format!("{base}/p.m3u8")).unwrap(), true).unwrap_err();
    assert_eq!(e.code, "drm", "{}", e.message);
}
