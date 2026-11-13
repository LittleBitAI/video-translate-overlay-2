//! A fragmented MP4 downloaded as one file (no HLS, no sections index) is
//! timed as an HLS section is: each fragment at its own `tfdt` and the audio
//! track's edit list, so a gap between fragments stays silence and a late
//! audio track stays late (R4). The fixtures are the HLS ones, each init
//! section and its segments joined into one file, as a server serving
//! fragmented MP4 sends it.

use crate::audio::RATE;
use std::path::Path;

/// `files` of `tests/fixtures/<dir>` joined into one download and decoded.
fn direct(dir: &str, files: &[&str]) -> Vec<i16> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(dir);
    let bytes: Vec<u8> = files.iter().flat_map(|f| std::fs::read(dir.join(f)).unwrap()).collect();
    let path = std::env::temp_dir().join(format!("vto-direct-{}-{}.mp4", files.join("-"), std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    assert!(!crate::media::sections_path(&path).exists());
    let pcm = crate::audio::decode_file(&path);
    let _ = std::fs::remove_file(&path);
    pcm.unwrap()
}

fn loud(pcm: &[i16], from: f64, to: f64) -> usize {
    let at = |s: f64| ((s * RATE as f64) as usize).min(pcm.len());
    pcm[at(from)..at(to)].iter().filter(|s| s.unsigned_abs() > 2_000).count()
}

fn tone_starts(pcm: &[i16]) -> f64 {
    pcm.iter().position(|s| s.unsigned_abs() > 2_000).expect("no tone") as f64 / RATE as f64
}

#[test]
fn a_gap_between_fragments_of_a_downloaded_file_stays_silence() {
    let pcm = direct("hls_gap", &["init.mp4", "seg0.m4s", "seg1-late.m4s"]);
    assert!(loud(&pcm, 0.2, 1.9) > 0, "the first fragment's tone is missing");
    assert_eq!(loud(&pcm, 2.3, 11.9), 0, "the second fragment's tone was pulled into the gap");
    assert!(loud(&pcm, 12.3, 13.9) > 0, "the second fragment's tone is not at 12 s ({} s decoded)", pcm.len() / RATE as usize);
}

#[test]
fn a_late_audio_track_of_a_downloaded_file_stays_late() {
    for (init, seg) in [("init.mp4", "seg0.m4s"), ("init-tfdt.mp4", "seg0-tfdt.m4s")] {
        let at = tone_starts(&direct("hls_lead", &[init, seg]));
        assert!((1.9..2.2).contains(&at), "{seg}: the tone starts at {at:.3} s, not 2 s");
    }
}
