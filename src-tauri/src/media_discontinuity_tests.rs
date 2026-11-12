//! An HLS playlist with `EXT-X-DISCONTINUITY` restarts its timestamps; players
//! place the next section where the EXTINFs put it, and so does the decoder
//! (R4, R17). `tests/fixtures/hls_disc` (ffmpeg): `seg0.ts` a 4 s tone stamped
//! from 0; `seg1.ts` 1 s of silence then a 3 s tone, stamped from 10 s; the
//! playlist gives each 4 s with a discontinuity between them.

use super::*;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Serves the files of `dir` (and `playlist` as `/index.m3u8`) until the process ends.
fn serve(dir: &'static Path, playlist: &'static str) -> String {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in server.incoming() {
            let Ok(s) = s else { continue };
            let mut r = BufReader::new(s.try_clone().unwrap());
            let (mut path, mut range) = (String::new(), false);
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                    break;
                }
                if let Some(p) = l.strip_prefix("GET ") {
                    path = p.split(' ').next().unwrap_or("").to_string();
                }
                range |= l.to_ascii_lowercase().starts_with("range:");
            }
            let body = match path.as_str() {
                "/index.m3u8" => Some(playlist.as_bytes().to_vec()),
                p => std::fs::read(dir.join(p.trim_start_matches('/'))).ok(),
            };
            let (mut status, mut body) = body.map_or((404, Vec::new()), |b| (200, b));
            // The probe asks one byte of each end segment.
            if range && status == 200 && path != "/index.m3u8" {
                (status, body) = (206, body[..1].to_vec());
            }
            let head = format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = (&s).write_all(head.as_bytes());
            let _ = (&s).write_all(&body);
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn dir() -> &'static Path {
    Box::leak(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hls_disc").into_boxed_path())
}

fn rms(pcm: &[i16], from: f64, to: f64) -> f64 {
    let s = &pcm[(from * 16_000.0) as usize..(to * 16_000.0) as usize];
    (s.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / s.len() as f64).sqrt()
}

#[test]
fn a_section_after_a_discontinuity_plays_where_the_playlist_puts_it() {
    let playlist = std::fs::read_to_string(dir().join("index.m3u8")).unwrap();
    let base = serve(dir(), Box::leak(playlist.into_boxed_str()));
    let src = probe(&Url::parse(&format!("{base}/index.m3u8")).unwrap(), true).unwrap();
    let Source::Hls { sections, .. } = &src else { panic!("not HLS") };
    assert_eq!(sections, &[Section { part: 0, at: 0.0 }, Section { part: 1, at: 4.0 }]);

    let dest = std::env::temp_dir().join(format!("vto-disc-{}.ts", std::process::id()));
    download(&src, &dest, true, &mut |_| {}).unwrap();
    let pcm = crate::audio::decode_file(&dest).unwrap();
    let secs = pcm.len() as f64 / 16_000.0;
    assert!((secs - 8.0).abs() < 0.15, "{secs:.2} s long, not the playlist's 8 s");
    assert!(rms(&pcm, 0.3, 3.7) > 1_500.0, "no tone in the first section");
    assert!(rms(&pcm, 4.2, 4.8) < 50.0, "the second section's silence is not at 4-5 s");
    let onset = 4.2 + pcm[(4.2 * 16_000.0) as usize..].iter().position(|s| s.unsigned_abs() > 3_000).unwrap() as f64 / 16_000.0;
    assert!((onset - 5.0).abs() < 0.1, "the second section's tone starts at {onset:.2} s, not 5 s");

    // The same file name downloaded again from a playlist without one loses the old index.
    let plain = Source::Hls { parts: vec![Part { url: format!("{base}/seg0.ts"), range: None }], ext: "ts", sections: vec![] };
    download(&plain, &dest, true, &mut |_| {}).unwrap();
    assert!(!sections_path(&dest).exists(), "a stale sections index was kept");
    let _ = std::fs::remove_file(&dest);
}

#[test]
fn sections_start_after_the_init_section_and_count_only_segments() {
    let base = Url::parse("https://cdn.example.org/v/index.m3u8").unwrap();
    let fmp4 = "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:6.5,\na.m4s\n#EXTINF:3.5,\nb.m4s\n\
                #EXT-X-DISCONTINUITY\n#EXTINF:4,\nc.m4s\n#EXT-X-DISCONTINUITY-SEQUENCE:2\n#EXTINF:4,\nd.m4s\n#EXT-X-ENDLIST";
    let Source::Hls { sections, .. } = probe_hls(&base, fmp4, true, 0).unwrap() else { panic!("not HLS") };
    assert_eq!(sections, [Section { part: 1, at: 0.0 }, Section { part: 3, at: 10.0 }]);
    // A discontinuity before the first segment changes nothing.
    let leading = "#EXTM3U\n#EXT-X-DISCONTINUITY\n#EXTINF:4,\na.ts\n#EXTINF:4,\nb.ts\n#EXT-X-ENDLIST";
    let Source::Hls { sections, .. } = probe_hls(&base, leading, true, 0).unwrap() else { panic!("not HLS") };
    assert!(sections.is_empty());
}
