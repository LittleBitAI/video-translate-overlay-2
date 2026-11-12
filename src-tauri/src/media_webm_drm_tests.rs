//! A WebM whose track is encrypted (`ContentEncryption` under its
//! `ContentEncodings`, as WebM's Common Encryption writes it) is refused as
//! `drm` before a job is made (R20, P6's 422); the same file in the clear is taken.

use super::tracks_tests::serve_dir;
use super::*;

/// An EBML element: its ID, its size (one byte when small, as muxers write it, else eight), its body.
fn el(id: &[u8], body: &[u8]) -> Vec<u8> {
    let mut e = id.to_vec();
    if body.len() < 127 {
        e.push(0x80 | body.len() as u8);
    } else {
        e.push(0x01);
        e.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
    }
    e.extend_from_slice(body);
    e
}

/// A WebM head with one Opus track, encrypted or not, and some cluster bytes after it.
fn webm(encrypted: bool) -> Vec<u8> {
    let header = el(&[0x1A, 0x45, 0xDF, 0xA3], &el(&[0x42, 0x82], b"webm"));
    let mut entry = [el(&[0xD7], &[1]), el(&[0x83], &[2]), el(&[0x86], b"A_OPUS")].concat();
    if encrypted {
        let encryption = el(&[0x50, 0x35], &[el(&[0x47, 0xE1], &[5]), el(&[0x47, 0xE2], &[0x11; 16])].concat());
        let encoding = el(&[0x62, 0x40], &[el(&[0x50, 0x33], &[1]), encryption].concat());
        entry.extend(el(&[0x6D, 0x80], &encoding));
    }
    let tracks = el(&[0x16, 0x54, 0xAE, 0x6B], &el(&[0xAE], &entry));
    let segment = el(&[0x18, 0x53, 0x80, 0x67], &[tracks, el(&[0x1F, 0x43, 0xB6, 0x75], &[0u8; 512])].concat());
    [header, segment].concat()
}

#[test]
fn an_encrypted_webm_is_refused_before_a_job() {
    let base = serve_dir("tracks", vec![("/locked.webm".into(), webm(true)), ("/clear.webm".into(), webm(false))]);
    let probe_at = |path: &str| probe(&Url::parse(&format!("{base}{path}")).unwrap(), true);
    assert_eq!(probe_at("/locked.webm").unwrap_err().code, "drm");
    assert!(matches!(probe_at("/clear.webm"), Ok(Source::File { ext: "webm", .. })), "the same file in the clear");
    assert!(webm_encrypted(webm_tracks(&webm(true)).unwrap()));
    assert!(!webm_encrypted(webm_tracks(&webm(false)).unwrap()));
    // A real WebM with video and Opus audio is still taken.
    assert!(matches!(probe_at("/with-audio.webm"), Ok(Source::File { ext: "webm", .. })));
}
