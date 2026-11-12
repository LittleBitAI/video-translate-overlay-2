//! Probing against a real (loopback) HTTP server: redirects and MP4 boxes.

use super::*;
use std::io::{BufRead, BufReader};

/// Serves `route(path)` = `(status, extra header lines, body)` until the test
/// process ends, answering `Range` requests with 206.
fn serve(route: impl Fn(&str) -> (u16, String, Vec<u8>) + Send + 'static) -> String {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in server.incoming() {
            let Ok(s) = s else { continue };
            let mut r = BufReader::new(s.try_clone().unwrap());
            let (mut path, mut range) = (String::new(), None::<(usize, usize)>);
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                    break;
                }
                if let Some(p) = l.strip_prefix("GET ") {
                    path = p.split(' ').next().unwrap_or("").to_string();
                }
                if let Some(v) = l.to_ascii_lowercase().trim().strip_prefix("range: bytes=") {
                    let (a, b) = v.split_once('-').unwrap();
                    range = Some((a.parse().unwrap(), b.parse().unwrap()));
                }
            }
            let (mut status, extra, mut body) = route(&path);
            if let (200, Some((a, b))) = (status, range) {
                if a >= body.len() {
                    status = 416;
                    body.clear();
                } else {
                    body = body[a..=b.min(body.len() - 1)].to_vec();
                    status = 206;
                }
            }
            let head = format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n", body.len());
            let _ = (&s).write_all(head.as_bytes());
            let _ = (&s).write_all(&body);
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn redirect(to: &str) -> (u16, String, Vec<u8>) {
    (302, format!("Location: {to}\r\n"), Vec::new())
}

fn playlist(body: &str) -> (u16, String, Vec<u8>) {
    (200, "Content-Type: application/vnd.apple.mpegurl\r\n".into(), body.as_bytes().to_vec())
}

#[test]
fn a_redirected_playlist_resolves_against_where_it_landed() {
    let vod = "#EXTM3U\n#EXTINF:4,\nseg0.aac\n#EXT-X-ENDLIST\n";
    let base = serve(move |path| match path {
        "/entry.m3u8" => redirect("/vod/index.m3u8"),
        "/vod/index.m3u8" | "/media/a/index.m3u8" => playlist(vod),
        "/master.m3u8" => playlist("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nv/redir.m3u8\n"),
        "/v/redir.m3u8" => redirect("/media/a/index.m3u8"),
        // The probe asks a segment for a byte: only those where the playlists landed exist.
        "/vod/seg0.aac" | "/media/a/seg0.aac" => (200, String::new(), vec![0xFF, 0xF1, 0x50]),
        _ => (404, String::new(), Vec::new()),
    });
    let first = |path: &str| match probe(&Url::parse(&format!("{base}{path}")).unwrap(), true).unwrap() {
        Source::Hls { parts, .. } => parts[0].url.clone(),
        other => panic!("not HLS: {other:?}"),
    };
    assert_eq!(first("/entry.m3u8"), format!("{base}/vod/seg0.aac"));
    // A variant playlist that redirects resolves its segments where it landed too.
    assert_eq!(first("/master.m3u8"), format!("{base}/media/a/seg0.aac"));
}

fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    b.extend_from_slice(kind);
    b.extend_from_slice(body);
    b
}

/// `moov` with one audio track whose sample entry is `entry` holding `inner`.
fn moov(entry: &[u8; 4], inner: &[u8]) -> Vec<u8> {
    let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
    stsd.extend(mp4_box(entry, &[&[0u8; 28][..], inner].concat()));
    let stbl = mp4_box(b"stbl", &mp4_box(b"stsd", &stsd));
    let trak = mp4_box(b"trak", &mp4_box(b"mdia", &mp4_box(b"minf", &stbl)));
    mp4_box(b"moov", &trak)
}

#[test]
fn protected_mp4_files_are_refused_before_a_job() {
    let ftyp = mp4_box(b"ftyp", b"isom\0\0\0\0isomiso6");
    let sinf = mp4_box(b"sinf", &[mp4_box(b"frma", b"mp4a"), mp4_box(b"schi", &mp4_box(b"tenc", &[0; 24]))].concat());
    let protected = moov(b"enca", &sinf);
    let clear = moov(b"mp4a", &[]);
    // An `mdat` larger than the probe's first read, whose samples happen to spell a mark.
    let mdat = mp4_box(b"mdat", &[b"sinf".repeat(10), vec![0; 100_000]].concat());
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("/fast.mp4", [&ftyp[..], &protected, &mdat].concat()),
        ("/late.mp4", [&ftyp[..], &mdat, &protected].concat()),
        ("/clear.mp4", [&ftyp[..], &mdat, &clear].concat()),
        ("/clear-fast.mp4", [&ftyp[..], &clear, &mdat].concat()),
    ];
    let base = serve(move |path| match files.iter().find(|(p, _)| *p == path) {
        Some((_, body)) => (200, "Content-Type: video/mp4\r\n".into(), body.clone()),
        None => (404, String::new(), Vec::new()),
    });
    let probe_at = |path: &str| probe(&Url::parse(&format!("{base}{path}")).unwrap(), true);
    assert_eq!(probe_at("/fast.mp4").unwrap_err().code, "drm");
    assert_eq!(probe_at("/late.mp4").unwrap_err().code, "drm", "moov after mdat is read by range");
    for clear in ["/clear.mp4", "/clear-fast.mp4"] {
        assert!(matches!(probe_at(clear), Ok(Source::File { ext: "m4a", .. })), "{clear}");
    }
}
