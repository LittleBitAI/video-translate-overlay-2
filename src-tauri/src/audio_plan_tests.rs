//! Plan 2: windows as full as the model takes, cut at pauses, and overlapping
//! only where speech runs on with no pause near the limit.

use super::*;

/// Speech-like noise (random, loud) over `spans` (ms), quiet noise elsewhere.
fn talk(total_ms: usize, spans: &[(usize, usize)], peak: i32) -> Vec<i16> {
    let mut x = 7u32;
    (0..total_ms * RATE / 1000)
        .map(|i| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let r = (x >> 16) as i32 % 2001 - 1000; // -1000..=1000
            let ms = i * 1000 / RATE;
            let loud = spans.iter().any(|&(a, b)| (a..b).contains(&ms));
            (if loud { r * peak / 1000 } else { r / 200 }) as i16
        })
        .collect()
}

fn ms(r: &[Region]) -> Vec<(i64, i64)> {
    r.iter().map(|w| (w.start_ms(), w.end_ms())).collect()
}

fn near(got: i64, want: i64) -> bool {
    (got - want).abs() <= 60
}

#[test]
fn spans_apart_by_short_pauses_share_a_window_and_long_silence_ends_it() {
    // Pauses of 1 s (kept inside) and 10 s (a new window).
    let pcm = talk(40_000, &[(1_000, 5_000), (6_000, 10_000), (20_000, 24_000)], 8_000);
    let w = speech_windows(&pcm);
    assert_eq!(w.len(), 2, "{:?}", ms(&w));
    assert!(near(w[0].start_ms(), 800) && near(w[0].end_ms(), 10_200), "{:?}", ms(&w));
    assert!(near(w[1].start_ms(), 19_800) && near(w[1].end_ms(), 24_200), "{:?}", ms(&w));
    // Plan 1 had three, one per span.
    assert_eq!(plan(1, &pcm).len(), 3);
    assert_eq!(plan(PLAN, &pcm), w);
}

#[test]
fn long_speech_is_cut_at_the_latest_pause_before_the_limit_without_overlap() {
    // 70 s of talk with 300 ms pauses at 12 s, 25 s and 52 s.
    let pcm = talk(72_000, &[(1_000, 12_000), (12_300, 25_000), (25_300, 52_000), (52_300, 71_000)], 8_000);
    let w = speech_windows(&pcm);
    assert_eq!(w.len(), 3, "{:?}", ms(&w));
    assert!(w.windows(2).all(|p| p[0].end == p[1].start), "{:?}", ms(&w));
    assert!(near(w[0].end_ms(), 25_150) && near(w[1].end_ms(), 52_150), "{:?}", ms(&w));
    assert!(w.iter().all(|r| r.end - r.start <= MAX_REGION));
    assert!(near(w[0].start_ms(), 800) && near(w[2].end_ms(), 71_200));
}

#[test]
fn speech_with_no_pause_overlaps_the_next_window_by_the_overlap() {
    let pcm = talk(75_000, &[(1_000, 74_000)], 8_000);
    let w = speech_windows(&pcm);
    // Where the quietest 200 ms falls decides whether 73 s takes three windows or four.
    assert!(w.len() >= 3, "{:?}", ms(&w));
    for p in w.windows(2) {
        assert_eq!(p[0].end - p[1].start, OVERLAP, "{:?}", ms(&w));
    }
    assert!(w.iter().all(|r| r.end - r.start <= MAX_REGION), "{:?}", ms(&w));
    assert!(near(w[0].start_ms(), 800) && near(w.last().unwrap().end_ms(), 74_200), "{:?}", ms(&w));
}

#[test]
fn a_long_span_after_a_short_pause_starts_in_the_room_left() {
    // 8 s, a 1 s pause, then 40 s with a pause at 25 s: one cut, inside the long span.
    let pcm = talk(52_000, &[(1_000, 9_000), (10_000, 25_000), (25_300, 50_000)], 8_000);
    let w = speech_windows(&pcm);
    assert_eq!(w.len(), 2, "{:?}", ms(&w));
    assert!(near(w[0].start_ms(), 800) && near(w[0].end_ms(), 25_150), "{:?}", ms(&w));
    assert_eq!(w[0].end, w[1].start);
}

#[test]
fn a_short_pause_after_any_fill_of_a_window_plans_bounded_windows_over_all_speech() {
    // A first span filling the window up to (and past) its limit, a pause short
    // enough to join, then a span too long for one window.
    for first_end in [20_000, 25_000, 29_000, 30_000, 30_500, 31_000] {
        for gap in [300, 800, 1_200] {
            let spans = [(1_000, first_end), (first_end + gap, first_end + gap + 40_000)];
            let pcm = talk(first_end + gap + 42_000, &spans, 8_000);
            let w = speech_windows(&pcm);
            let case = format!("first span to {first_end} ms, pause {gap} ms: {:?}", ms(&w));
            assert!(w.iter().all(|r| r.start < r.end && r.end - r.start <= MAX_REGION), "{case}");
            assert!(w.windows(2).all(|p| p[0].start < p[1].start && p[0].end <= p[1].end && p[1].start <= p[0].end + MAX_GAP), "{case}");
            for &(a, b) in &spans {
                let heard = |t: usize| w.iter().any(|r| (r.start_ms() as usize..r.end_ms() as usize).contains(&t));
                assert!((a..b).step_by(100).all(heard), "speech in {a}..{b} left out; {case}");
            }
        }
    }
    // The tuning harness's overlap, however long, is bounded by the search for a pause.
    let pcm = talk(75_000, &[(1_000, 74_000)], 8_000);
    for overlap_s in [0, 3, 10, 20, 60] {
        let w = speech_windows_tuned(&pcm, overlap_s * RATE, false);
        assert!(w.iter().all(|r| r.start < r.end && r.end - r.start <= MAX_REGION), "overlap {overlap_s} s: {:?}", ms(&w));
        assert!(w.windows(2).all(|p| p[0].start < p[1].start), "overlap {overlap_s} s: {:?}", ms(&w));
    }
}

#[test]
fn quiet_speech_gets_windows_and_silence_none() {
    let spans = [(1_000, 3_000), (4_000, 6_000)];
    let quiet = talk(10_000, &spans, 600);
    assert_eq!(ms(&speech_windows(&quiet)), ms(&speech_windows(&talk(10_000, &spans, 8_000))));
    assert_eq!(speech_windows(&quiet).len(), 1);
    assert!(speech_windows(&vec![0i16; 10 * RATE]).is_empty());
}
