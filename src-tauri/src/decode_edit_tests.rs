//! An MP4 edit list played through: delays, skips and repeats land each
//! stretch of the track where the player plays it.

use super::*;

/// One sample per 1/16 s: track sample `i` holds `i`, so where it lands shows where it came from.
fn track(secs: usize) -> Vec<i16> {
    (0..secs * RATE).map(|i| (i / (RATE / 16)) as i16 + 1).collect()
}

fn at(pcm: &[i16], secs: f64) -> i16 {
    pcm[(secs * RATE as f64) as usize]
}

#[test]
fn delays_skips_and_repeats_land_where_the_player_plays_them() {
    let t = track(10);
    let edits = [
        Edit { from: None, secs: 1.0 },          // 0-1 s: silence
        Edit { from: Some(0.0), secs: 2.0 },     // 1-3 s: track 0-2 s
        Edit { from: Some(7.0), secs: 1.0 },     // 3-4 s: track 7-8 s (2-7 s skipped)
        Edit { from: Some(0.0), secs: 1.0 },     // 4-5 s: track 0-1 s again
        Edit { from: Some(9.5), secs: 1.0 },     // 5-6 s: track 9.5-10 s, then silence
    ];
    let out = apply_edits(&t, &edits).unwrap();
    assert_eq!(out.len(), 6 * RATE);
    assert_eq!(at(&out, 0.5), 0);
    assert_eq!(at(&out, 1.0), at(&t, 0.0));
    assert_eq!(at(&out, 2.5), at(&t, 1.5));
    assert_eq!(at(&out, 3.25), at(&t, 7.25), "a skipped part does not push later audio late");
    assert_eq!(at(&out, 4.5), at(&t, 0.5), "a repeated part plays again");
    assert_eq!(at(&out, 5.25), at(&t, 9.75));
    assert_eq!(at(&out, 5.75), 0, "past the end of the track is silence");
}

#[test]
fn a_media_edit_of_length_zero_runs_to_the_end() {
    let t = track(4);
    let out = apply_edits(&t, &[Edit { from: Some(1.0), secs: 0.0 }]).unwrap();
    assert_eq!(out, t[RATE..]);
}
