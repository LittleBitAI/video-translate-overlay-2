//! Audio keeps its place on the video's timeline (R4, E3): a stream whose audio
//! stops while the video runs on decodes with the gap kept, not closed up.
//! `gap.ts` (ffmpeg): video for 14 s; a 440 Hz tone at 0-2 s and again at 12-14 s,
//! with no audio packets between.

use super::RATE;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio").join(name)
}

fn rms(pcm: &[i16], from: f64, to: f64) -> f64 {
    let s = &pcm[(from * RATE as f64) as usize..(to * RATE as f64) as usize];
    (s.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / s.len() as f64).sqrt()
}

/// When the tone comes back after the gap, in seconds.
fn second_onset(pcm: &[i16]) -> f64 {
    let from = 5 * RATE;
    let i = pcm[from..].iter().position(|s| s.unsigned_abs() > 3_000).expect("no audio after the gap");
    (from + i) as f64 / RATE as f64
}

fn assert_gap_kept(name: &str, pcm: &[i16]) {
    let secs = pcm.len() as f64 / RATE as f64;
    assert!((13.8..14.4).contains(&secs), "{name}: {secs:.2} s long");
    assert!(rms(pcm, 0.3, 1.7) > 1_500.0, "{name}: no tone at the start");
    assert!(rms(pcm, 3.0, 11.0) < 50.0, "{name}: the gap is not silent");
    assert!(rms(pcm, 12.3, 13.7) > 1_500.0, "{name}: no tone after the gap");
    let onset = second_onset(pcm);
    assert!((onset - 12.0).abs() < 0.15, "{name}: the second tone starts at {onset:.3} s, not 12 s");
}

/// Through the app's entry point, so it holds on Windows (where MPEG-TS skips
/// Media Foundation) as well as elsewhere.
#[test]
fn a_gap_in_ts_audio_is_kept() {
    let pcm = super::decode_file(&fixture("gap.ts")).unwrap();
    assert_gap_kept("gap.ts", &pcm);
}
