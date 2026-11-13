//! A media URL that serves something no decoder reads (an image, a document)
//! is refused before a job is made (R20, P6's 422), not downloaded and failed.

use super::*;
use std::io::{BufRead, BufReader};

/// Serves `files` (path, content type, body) until the process ends.
fn serve(files: Vec<(&'static str, &'static str, Vec<u8>)>) -> String {
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
            let (ct, body) = files.iter().find(|(p, ..)| *p == path).map_or(("text/plain", &[][..]), |(_, ct, b)| (*ct, &b[..]));
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = (&s).write_all(head.as_bytes());
            let _ = (&s).write_all(body);
        }
    });
    format!("http://127.0.0.1:{port}")
}

#[test]
fn an_image_or_a_document_is_refused_before_a_job() {
    let png = [&b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"[..], &[0; 64]].concat();
    let pdf = b"%PDF-1.7\n1 0 obj\n<< >>\nendobj\n".to_vec();
    let mut wav = b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x80\x3e\0\0\0\x7d\0\0\x02\0\x10\0data\0\0\0\0".to_vec();
    wav.resize(64, 0);
    let base = serve(vec![
        ("/talk.png", "image/png", png),
        // A name and a type that say audio do not make it audio.
        ("/talk.mp3", "audio/mpeg", pdf),
        ("/talk.wav", "audio/wav", wav),
    ]);
    let probe_at = |path: &str| probe(&Url::parse(&format!("{base}{path}")).unwrap(), true);
    assert_eq!(probe_at("/talk.png").unwrap_err().code, "not_audio");
    assert_eq!(probe_at("/talk.mp3").unwrap_err().code, "not_audio");
    assert!(matches!(probe_at("/talk.wav"), Ok(Source::File { ext: "wav", .. })));
}
