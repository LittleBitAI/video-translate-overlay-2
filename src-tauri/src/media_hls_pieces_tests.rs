//! An HLS playlist whose pieces do not answer cannot be downloaded: the probe
//! refuses it before a job is made (R20, P6's 422), as with any other media.

use super::*;
use std::io::{BufRead, BufReader};

/// Serves `files` (path, body) and 404 for anything else, until the process ends.
fn serve(files: Vec<(&'static str, &'static [u8])>) -> String {
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
            let (status, body) = files.iter().find(|(p, _)| *p == path).map_or((404, &b""[..]), |(_, b)| (200, *b));
            let head = format!("HTTP/1.1 {status} X\r\nContent-Type: application/vnd.apple.mpegurl\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = (&s).write_all(head.as_bytes());
            let _ = (&s).write_all(body);
        }
    });
    format!("http://127.0.0.1:{port}")
}

#[test]
fn a_playlist_whose_segments_do_not_answer_is_refused() {
    let base = serve(vec![
        ("/gone/index.m3u8", b"#EXTM3U\n#EXTINF:4,\nmissing.aac\n#EXT-X-ENDLIST\n"),
        ("/last/index.m3u8", b"#EXTM3U\n#EXTINF:4,\nseg0.aac\n#EXTINF:4,\nmissing.aac\n#EXT-X-ENDLIST\n"),
        ("/last/seg0.aac", b"\xFF\xF1\x50"),
        ("/ok/index.m3u8", b"#EXTM3U\n#EXTINF:4,\nseg0.aac\n#EXTINF:4,\nseg1.aac\n#EXT-X-ENDLIST\n"),
        ("/ok/seg0.aac", b"\xFF\xF1\x50"),
        ("/ok/seg1.aac", b"\xFF\xF1\x50"),
    ]);
    let probe_at = |path: &str| probe(&Url::parse(&format!("{base}{path}")).unwrap(), true);
    assert_eq!(probe_at("/gone/index.m3u8").unwrap_err().code, "segment_unavailable");
    assert_eq!(probe_at("/last/index.m3u8").unwrap_err().code, "segment_unavailable", "the last segment is asked too");
    assert!(matches!(probe_at("/ok/index.m3u8"), Ok(Source::Hls { ext: "aac", .. })));
}
