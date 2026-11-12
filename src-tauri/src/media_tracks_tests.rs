//! An HLS playlist that changes its init section (`EXT-X-MAP`) decodes each
//! section after its own one (R4, R17), and a section that does not decode
//! fails the job rather than leaving a stretch unwritten. A file with no sound
//! track is refused before a job is made (R20, P6's 422).
//!
//! `tests/fixtures/hls_maps` (ffmpeg, fMP4): `init-a.mp4` + `a0.m4s`, a 4 s
//! 440 Hz tone at 44.1 kHz on track 1; `init-b.mp4` + `b0.m4s`, 1 s of silence
//! then a 3 s 660 Hz tone at 48 kHz on track 2, stamped from 10 s.
//! `tests/fixtures/tracks`: 1 s video-only MP4, MPEG-TS and WebM, and a WebM
//! with video and Opus audio.

use super::*;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

fn fixtures(dir: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(dir)
}

/// Serves `files` (path, body), with `Range: bytes=a-b` answered 206, until the process ends.
fn serve(files: Vec<(String, Vec<u8>)>) -> String {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in server.incoming() {
            let Ok(s) = s else { continue };
            let mut r = BufReader::new(s.try_clone().unwrap());
            let (mut path, mut range) = (String::new(), None);
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                    break;
                }
                if let Some(p) = l.strip_prefix("GET ") {
                    path = p.split(' ').next().unwrap_or("").to_string();
                }
                if let Some(v) = l.to_ascii_lowercase().strip_prefix("range: bytes=") {
                    let (a, b) = v.trim().split_once('-').unwrap();
                    range = Some((a.parse::<usize>().unwrap(), b.parse::<usize>().ok()));
                }
            }
            let (status, body) = match (files.iter().find(|(p, _)| *p == path), range) {
                (None, _) => (404, Vec::new()),
                (Some((_, b)), Some((from, to))) if from < b.len() => (206, b[from..to.map_or(b.len(), |t| (t + 1).min(b.len()))].to_vec()),
                (Some(_), Some(_)) => (416, Vec::new()),
                (Some((_, b)), None) => (200, b.clone()),
            };
            let head = format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = (&s).write_all(head.as_bytes());
            let _ = (&s).write_all(&body);
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn serve_dir(dir: &str, extra: Vec<(String, Vec<u8>)>) -> String {
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(fixtures(dir))
        .unwrap()
        .map(|e| e.unwrap().path())
        .map(|p| (format!("/{}", p.file_name().unwrap().to_str().unwrap()), std::fs::read(&p).unwrap()))
        .collect();
    files.extend(extra);
    serve(files)
}

fn rms(pcm: &[i16], from: f64, to: f64) -> f64 {
    let s = &pcm[(from * 16_000.0) as usize..(to * 16_000.0) as usize];
    (s.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / s.len() as f64).sqrt()
}

fn download_and_decode(src: &Source, name: &str) -> Result<Vec<i16>, String> {
    let dest = std::env::temp_dir().join(format!("vto-maps-{}-{name}.mp4", std::process::id()));
    download(src, &dest, true, &mut |_| {}).unwrap();
    let pcm = crate::audio::decode_file(&dest);
    let _ = std::fs::remove_file(sections_path(&dest));
    let _ = std::fs::remove_file(&dest);
    pcm
}

#[test]
fn a_section_with_its_own_init_section_is_decoded_after_that_one() {
    let base = serve_dir("hls_maps", vec![]);
    let src = probe(&Url::parse(&format!("{base}/index.m3u8")).unwrap(), true).unwrap();
    let Source::Hls { parts, sections, .. } = &src else { panic!("not HLS") };
    let names: Vec<_> = parts.iter().map(|p| p.url.rsplit('/').next().unwrap()).collect();
    assert_eq!(names, ["init-a.mp4", "a0.m4s", "init-b.mp4", "b0.m4s"]);
    assert_eq!(sections, &[Section { part: 1, at: 0.0, init: Some(0) }, Section { part: 3, at: 4.0, init: Some(2) }]);

    let pcm = download_and_decode(&src, "ok").unwrap();
    let secs = pcm.len() as f64 / 16_000.0;
    assert!((secs - 8.0).abs() < 0.15, "{secs:.2} s long, not the playlist's 8 s");
    assert!(rms(&pcm, 0.3, 3.7) > 1_500.0, "no tone in the first section");
    assert!(rms(&pcm, 4.2, 4.8) < 50.0, "the second section's silence is not at 4-5 s");
    assert!(rms(&pcm, 5.3, 7.7) > 1_500.0, "the second section (its own track and rate) is missing");
    let onset = 4.2 + pcm[(4.2 * 16_000.0) as usize..].iter().position(|s| s.unsigned_abs() > 3_000).unwrap() as f64 / 16_000.0;
    assert!((onset - 5.0).abs() < 0.1, "the second section's tone starts at {onset:.2} s, not 5 s");
    // 660 Hz over a second: 1,320 sign changes; 44.1 kHz settings on 48 kHz data would not give it.
    let mid = &pcm[6 * 16_000..7 * 16_000];
    let crossings = mid.windows(2).filter(|w| (w[0] < 0) != (w[1] < 0)).count();
    assert!((1_300..=1_340).contains(&crossings), "{crossings} crossings, not a 660 Hz tone");
}

#[test]
fn a_section_that_does_not_decode_fails_the_whole_file() {
    let playlist = std::fs::read_to_string(fixtures("hls_maps").join("index.m3u8")).unwrap().replace("b0.m4s", "junk.m4s");
    let junk: Vec<u8> = (0..30_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8).collect();
    let base = serve_dir("hls_maps", vec![("/junk.m3u8".into(), playlist.into_bytes()), ("/junk.m4s".into(), junk)]);
    let src = probe(&Url::parse(&format!("{base}/junk.m3u8")).unwrap(), true).unwrap();
    let e = download_and_decode(&src, "junk").unwrap_err();
    assert!(e.contains("section 2"), "{e}");
}

#[test]
fn an_init_section_switch_starts_a_section_and_a_repeat_does_not() {
    let base = Url::parse("https://cdn.example.org/v/index.m3u8").unwrap();
    let sections = |m3u8: &str| match probe_hls(&base, m3u8, true, 0).unwrap() {
        Source::Hls { parts, sections, .. } => (parts.len(), sections),
        _ => panic!("not HLS"),
    };
    // A new EXT-X-MAP with no EXT-X-DISCONTINUITY still starts a section.
    let switch = "#EXTM3U\n#EXT-X-MAP:URI=\"a.mp4\"\n#EXTINF:4,\n1.m4s\n#EXT-X-MAP:URI=\"b.mp4\"\n#EXTINF:4,\n2.m4s\n#EXT-X-ENDLIST";
    assert_eq!(sections(switch), (4, vec![Section { part: 1, at: 0.0, init: Some(0) }, Section { part: 3, at: 4.0, init: Some(2) }]));
    // The same EXT-X-MAP again is neither fetched again nor a section.
    let repeat = "#EXTM3U\n#EXT-X-MAP:URI=\"a.mp4\"\n#EXTINF:4,\n1.m4s\n#EXT-X-MAP:URI=\"a.mp4\"\n#EXTINF:4,\n2.m4s\n#EXT-X-ENDLIST";
    assert_eq!(sections(repeat), (3, vec![]));
    // An EXT-X-MAP after the last segment is not downloaded.
    let trailing = "#EXTM3U\n#EXTINF:4,\n1.m4s\n#EXT-X-MAP:URI=\"z.mp4\"\n#EXT-X-ENDLIST";
    assert_eq!(sections(trailing), (1, vec![]));
}

#[test]
fn a_file_with_no_sound_track_is_refused_before_a_job() {
    let audio = fixtures("audio");
    let extra = ["gap.mp4", "gap.ts", "tone.m4a", "tone.ts"].map(|n| (format!("/{n}"), std::fs::read(audio.join(n)).unwrap()));
    let base = serve_dir("tracks", extra.into());
    let probe_at = |path: &str| probe(&Url::parse(&format!("{base}{path}")).unwrap(), true);
    for silent in ["/video.mp4", "/video.ts", "/video.webm"] {
        assert_eq!(probe_at(silent).unwrap_err().code, "not_audio", "{silent}");
    }
    for (with_sound, ext) in [("/gap.mp4", "m4a"), ("/tone.m4a", "m4a"), ("/gap.ts", "ts"), ("/tone.ts", "ts"), ("/with-audio.webm", "webm")] {
        match probe_at(with_sound) {
            Ok(Source::File { ext: e, .. }) => assert_eq!(e, ext, "{with_sound}"),
            other => panic!("{with_sound}: {other:?}"),
        }
    }
}
