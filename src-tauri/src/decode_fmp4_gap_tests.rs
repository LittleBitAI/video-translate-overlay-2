//! Audio missing between two fragments of one fMP4 HLS section (no
//! discontinuity, the video running on) stays silence: each fragment plays at
//! its own `tfdt`, so the speech after the gap, and its cues, keep their time (R4).
//!
//! `tests/fixtures/hls_gap` (ffmpeg, two 2 s fMP4 segments, video on track 1
//! and a 440 Hz AAC tone on track 2 throughout): `seg1-late.m4s` is the second
//! segment with its audio's `tfdt` moved 10 s later, from 2.112 s to 12.112 s.

use crate::audio::RATE;
use std::path::Path;

fn loud(pcm: &[i16], from: f64, to: f64) -> usize {
    let at = |s: f64| ((s * RATE as f64) as usize).min(pcm.len());
    pcm[at(from)..at(to)].iter().filter(|s| s.unsigned_abs() > 2_000).count()
}

#[test]
fn a_gap_between_fragments_stays_silence() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hls_gap");
    let parts: Vec<Vec<u8>> = ["init.mp4", "seg0.m4s", "seg1-late.m4s"].iter().map(|f| std::fs::read(dir.join(f)).unwrap()).collect();
    let path = std::env::temp_dir().join(format!("vto-gap-{}.mp4", std::process::id()));
    std::fs::write(&path, parts.concat()).unwrap();
    let (init, total) = (parts[0].len(), parts.iter().map(Vec::len).sum::<usize>());
    let index = crate::media::sections_path(&path);
    std::fs::write(&index, format!("{init} {total} 0 0 {init}\n")).unwrap();
    let pcm = crate::audio::decode_file(&path);
    let _ = std::fs::remove_file(&index);
    let _ = std::fs::remove_file(&path);
    let pcm = pcm.unwrap();

    assert!(loud(&pcm, 0.2, 1.9) > 0, "the first fragment's tone is missing");
    assert_eq!(loud(&pcm, 2.3, 11.9), 0, "the second fragment's tone was pulled into the gap");
    assert!(loud(&pcm, 12.3, 13.9) > 0, "the second fragment's tone is not at 12 s ({} s decoded)", pcm.len() / RATE as usize);
}

#[test]
fn ticks_jump_from_where_each_fragment_starts() {
    let jumps = [(100, 50), (300, -20)];
    assert_eq!([0, 99, 100, 299, 300, 9_999].map(|t| super::jump_at(&jumps, t)), [0, 0, 50, 50, -20, -20]);
    assert_eq!(super::jump_at(&[], 7), 0);
}
