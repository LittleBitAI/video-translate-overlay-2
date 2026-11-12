//! An fMP4 HLS section whose audio track starts after its video track keeps
//! that lead: players start the section where its earliest track begins, so
//! the speech, and every cue after it, comes as late as the player plays it (R4).
//!
//! `tests/fixtures/hls_lead` (ffmpeg, one fMP4 segment, video on track 1 from
//! 0 s, a 440 Hz AAC tone on track 2 from 2 s): `init.mp4` + `seg0.m4s` give the
//! audio's lead as an empty edit in its edit list, as ffmpeg writes it, with
//! both `tfdt`s 0; `init-tfdt.mp4` + `seg0-tfdt.m4s` are the same with the empty
//! edit at 0 s and the audio's `tfdt` at 2 s instead.

use crate::audio::RATE;
use std::path::Path;

/// When the tone is first heard in `init` + `seg` downloaded as one HLS section.
fn tone_starts(name: &str, init: &str, seg: &str) -> f64 {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hls_lead");
    let (init, seg) = (std::fs::read(dir.join(init)).unwrap(), std::fs::read(dir.join(seg)).unwrap());
    let path = std::env::temp_dir().join(format!("vto-lead-{name}-{}.mp4", std::process::id()));
    std::fs::write(&path, [&init[..], &seg[..]].concat()).unwrap();
    let index = crate::media::sections_path(&path);
    std::fs::write(&index, format!("{} {} 0 0 {}\n", init.len(), init.len() + seg.len(), init.len())).unwrap();
    let pcm = crate::audio::decode_file(&path);
    let _ = std::fs::remove_file(&index);
    let _ = std::fs::remove_file(&path);
    let pcm = pcm.unwrap();
    let at = pcm.iter().position(|s| s.unsigned_abs() > 2_000).expect("no tone");
    at as f64 / RATE as f64
}

#[test]
fn audio_led_by_an_edit_list_starts_where_players_play_it() {
    let at = tone_starts("elst", "init.mp4", "seg0.m4s");
    assert!((1.9..2.2).contains(&at), "the tone starts at {at:.3} s, not 2 s");
}

#[test]
fn audio_led_by_its_tfdt_starts_where_players_play_it() {
    let at = tone_starts("tfdt", "init-tfdt.mp4", "seg0-tfdt.m4s");
    assert!((1.9..2.2).contains(&at), "the tone starts at {at:.3} s, not 2 s");
}
