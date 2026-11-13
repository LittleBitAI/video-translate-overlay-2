//! MP4 and WebM/Opus audio keep their place on the timeline too (R4, E3), and
//! WebM/Opus decodes at all, through the decoder every OS (Android included)
//! uses, not Media Foundation.
//! `gap.mp4`, `gap.webm` (ffmpeg): a 440 Hz tone at 0-2 s and 12-14 s, nothing
//! between; the MP4 has 14 s of video, its gap is one 10 s sample duration.
//! `lead.mp4`: 6 s of video, the tone at 3-5 s, placed by an empty edit.
//! `trim.mp4` is an ffmpeg file whose `elst` media time was set to 5 s by hand;
//! `skip.mp4` and `cut.mp4` ones whose `elst` was given a second edit by hand.

use super::decode_gap_tests::{assert_gap_kept, fixture, rms};
use super::RATE;

#[test]
fn a_gap_in_mp4_and_webm_audio_is_kept() {
    for name in ["gap.mp4", "gap.webm"] {
        let path = fixture(name);
        let pcm = crate::decode::decode(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_gap_kept(name, &pcm);
        assert_eq!(super::decode_file(&path).unwrap(), pcm, "{name}: the app's entry point takes the same path");
    }
}

#[test]
fn an_mp4_track_that_starts_late_starts_late() {
    let pcm = crate::decode::decode(&fixture("lead.mp4")).unwrap();
    assert!(rms(&pcm, 0.3, 2.7) < 50.0, "the lead is not silent");
    assert!(rms(&pcm, 3.3, 4.7) > 1_500.0, "no tone at 3-5 s");
    let onset = pcm.iter().position(|s| s.unsigned_abs() > 3_000).unwrap() as f64 / RATE as f64;
    assert!((onset - 3.0).abs() < 0.15, "the tone starts at {onset:.3} s, not 3 s");
}

/// `trim.mp4`: 5 s of silence then a 3 s tone in the track, and an edit list
/// whose one edit starts at media time 5 s, so a player plays the tone at 0.
#[test]
fn an_mp4_edit_that_trims_the_start_moves_the_speech_with_it() {
    let pcm = crate::decode::decode(&fixture("trim.mp4")).unwrap();
    let onset = pcm.iter().position(|s| s.unsigned_abs() > 3_000).unwrap() as f64 / RATE as f64;
    assert!(onset < 0.05, "the tone starts at {onset:.3} s, not where the player plays it (0 s)");
    assert!(rms(&pcm, 0.2, 2.8) > 1_500.0, "no tone at 0-3 s");
}

/// `cut.mp4`: a 10 s track with a tone at 0.5-1.5 s and 3-4 s, and an edit
/// list that plays media 0-2 s then 4 s of silence: the player never plays
/// the second tone, so it must not be written either.
#[test]
fn an_mp4_media_edit_ends_where_its_length_does() {
    let pcm = crate::decode::decode(&fixture("cut.mp4")).unwrap();
    let secs = pcm.len() as f64 / RATE as f64;
    assert!((secs - 6.0).abs() < 0.05, "{secs:.2} s long, not the 6 s the player plays");
    assert!(rms(&pcm, 0.7, 1.3) > 1_500.0, "no tone at 0.5-1.5 s");
    assert!(rms(&pcm, 2.0, 5.9) < 50.0, "audio past the media edit's length was kept");
}

/// `skip.mp4`: a 10 s track with a tone at 0.5-1.5 s and 7-8 s, and an edit
/// list that plays media 0-2 s then 7-10 s, so the player has the tones at
/// 0.5 s and 2 s, and 5 s in all.
#[test]
fn an_mp4_edit_list_that_skips_a_part_is_played_through() {
    let pcm = crate::decode::decode(&fixture("skip.mp4")).unwrap();
    let secs = pcm.len() as f64 / RATE as f64;
    assert!((secs - 5.0).abs() < 0.1, "{secs:.2} s long, not the 5 s the player plays");
    assert!(rms(&pcm, 0.7, 1.3) > 1_500.0, "no tone at 0.5-1.5 s");
    assert!(rms(&pcm, 1.6, 1.9) < 50.0, "the first edit's end is not silent");
    let onset = 1.8 + pcm[(1.8 * RATE as f64) as usize..].iter().position(|s| s.unsigned_abs() > 3_000).unwrap() as f64 / RATE as f64;
    assert!((onset - 2.0).abs() < 0.05, "media 7 s plays at {onset:.3} s, not 2 s");
    assert!(rms(&pcm, 3.2, 4.8) < 50.0, "after the second tone is not silent");
}
