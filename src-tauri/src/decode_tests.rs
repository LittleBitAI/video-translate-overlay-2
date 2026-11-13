//! The pure-Rust decoder used off Windows, on the formats downloads arrive in.
//! Fixtures: a 2 s, 440 Hz stereo tone from ffmpeg (`tests/fixtures/audio`).

use super::RATE;
use crate::decode::decode;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio").join(name)
}

/// Sign changes per second over the middle second: twice the frequency.
fn crossings(pcm: &[i16]) -> usize {
    let mid = &pcm[pcm.len() / 2 - RATE / 2..pcm.len() / 2 + RATE / 2];
    mid.windows(2).filter(|w| (w[0] < 0) != (w[1] < 0)).count()
}

#[test]
fn every_download_format_decodes_to_the_tone() {
    for name in ["tone.m4a", "tone.aac", "tone.mp3", "tone.ts"] {
        let pcm = decode(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        // Encoders add some priming and padding around the 2 s.
        assert!((2 * RATE..2 * RATE + RATE / 5).contains(&pcm.len()), "{name}: {} samples", pcm.len());
        assert!((870..=890).contains(&crossings(&pcm)), "{name}: {} crossings", crossings(&pcm));
        assert!(rms(&pcm) > 1_500.0, "{name}: rms {}", rms(&pcm));
    }
}

fn rms(pcm: &[i16]) -> f64 {
    (pcm.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / pcm.len() as f64).sqrt()
}

/// The same files through Media Foundation give the same signal (R2 on both platforms).
#[cfg(windows)]
#[test]
fn it_agrees_with_media_foundation() {
    for name in ["tone.m4a", "tone.mp3"] {
        let ours = decode(&fixture(name)).unwrap();
        let mf = super::platform::decode(&fixture(name)).unwrap();
        assert!(ours.len().abs_diff(mf.len()) < RATE / 10, "{name}: {} vs {}", ours.len(), mf.len());
        assert!(crossings(&ours).abs_diff(crossings(&mf)) <= 4, "{name}");
        let (a, b) = (rms(&ours), rms(&mf));
        assert!((a - b).abs() < b * 0.05, "{name}: rms {a} vs {b}");
    }
}

#[test]
fn a_real_hls_segment_decodes() {
    let seg = Path::new(env!("CARGO_MANIFEST_DIR")).join("../contract-tests/fixtures/hls/seg0.aac");
    assert!(!decode(&seg).unwrap().is_empty());
}

#[test]
fn junk_is_an_error_not_a_panic() {
    let dir = std::env::temp_dir().join(format!("vto-decode-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // TS sync bytes with garbage between them, and plain noise.
    let mut ts = vec![0u8; 188 * 20];
    for (i, b) in ts.iter_mut().enumerate() {
        *b = if i % 188 == 0 { 0x47 } else { (i * 31 % 251) as u8 };
    }
    for (name, bytes) in [("junk.ts", ts), ("junk.m4a", (0..5000).map(|i| (i * 7 % 256) as u8).collect())] {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        assert!(decode(&p).is_err(), "{name}");
    }
    let _ = std::fs::remove_dir_all(dir);
}
