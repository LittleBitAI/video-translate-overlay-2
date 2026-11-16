//! Cues of a window on the source timeline, and words two overlapping
//! windows both heard kept once.

use super::*;
use crate::audio::RATE;

fn region(start_ms: usize, end_ms: usize) -> Region {
    Region { start: start_ms * RATE / 1000, end: end_ms * RATE / 1000 }
}

/// A segment of words given as (start, end, text) in window time.
fn seg(words: &[(i64, i64, &str)]) -> Segment {
    let words: Vec<Word> = words.iter().map(|&(a, b, t)| Word { start_ms: a, end_ms: b, text: format!(" {t}") }).collect();
    Segment { start_ms: words[0].start_ms, end_ms: words.last().unwrap().end_ms, text: words.iter().map(|w| w.text.as_str()).collect(), words }
}

fn texts(cues: &[(i64, i64, String)]) -> Vec<&str> {
    cues.iter().map(|c| c.2.as_str()).collect()
}

/// Window `a` then window `b` (which starts inside `a`), through JSON in between
/// as a resumed job would have it.
fn two(a: Region, sa: Vec<Segment>, b: Region, sb: Vec<Segment>) -> Vec<(i64, i64, String)> {
    let mut prev_end = 0;
    let (mut cues, held) = window_cues(&a, sa, Vec::new(), None, Some(&b), &mut prev_end);
    let held: Vec<Piece> = serde_json::from_str(&serde_json::to_string(&held).unwrap()).unwrap();
    let (more, left) = window_cues(&b, sb, held, Some(&a), None, &mut prev_end);
    assert!(left.is_empty());
    cues.extend(more);
    for c in &cues {
        assert!(c.0 < c.1, "empty or reversed cue {c:?}");
    }
    for w in cues.windows(2) {
        assert!(w[0].1 <= w[1].0, "cues overlap: {w:?}");
    }
    cues
}

#[test]
fn cues_move_to_the_source_timeline_inside_their_window() {
    let r = region(10_000, 20_000);
    let mut prev_end = 10_500;
    let segs = vec![
        Segment { start_ms: 0, end_ms: 2_000, text: " early".into(), words: Vec::new() },
        Segment { start_ms: 3_000, end_ms: 12_000, text: " late".into(), words: Vec::new() },
        Segment { start_ms: 4_000, end_ms: 5_000, text: "  ".into(), words: Vec::new() },
    ];
    let (cues, held) = window_cues(&r, segs, Vec::new(), None, None, &mut prev_end);
    assert!(held.is_empty());
    // After the previous cue, and never past the window's end.
    assert_eq!(cues, vec![(10_500, 12_000, "early".into()), (13_000, 20_000, "late".into())]);
    assert_eq!(prev_end, 20_000);
}

#[test]
fn a_word_cut_by_the_window_edge_appears_once_and_the_split_sentence_is_one_cue() {
    // Overlap 28-30 s. The earlier window ends inside "architecture", heard as a fragment.
    let (a, b) = (region(0, 30_000), region(28_000, 58_000));
    let sa = vec![
        seg(&[(1_000, 1_400, "Hello")]),
        seg(&[(26_000, 26_400, "the"), (26_500, 27_400, "transformer"), (27_500, 28_300, "uses"), (28_400, 28_900, "self"), (29_000, 29_500, "attention"), (29_600, 30_000, "archi")]),
    ];
    let sb = vec![seg(&[(0, 200, "uses"), (400, 900, "self"), (1_000, 1_500, "attention"), (1_600, 2_400, "architecture"), (2_500, 3_000, "everywhere")])];
    let cues = two(a, sa, b, sb);
    assert_eq!(texts(&cues), ["Hello", "the transformer uses self attention architecture everywhere"]);
    assert_eq!((cues[1].0, cues[1].1), (26_000, 31_000));
}

#[test]
fn natural_repetition_across_the_cut_keeps_every_spoken_repeat() {
    // "very" is said four times; the overlap holds the last two and a fragment.
    let (a, b) = (region(0, 30_000), region(28_000, 58_000));
    let sa = vec![seg(&[(27_000, 27_400, "very"), (27_500, 27_900, "very"), (28_200, 28_600, "very"), (29_100, 29_500, "very"), (29_800, 30_000, "go")])];
    let sb = vec![seg(&[(200, 600, "very"), (1_100, 1_500, "very"), (1_700, 2_200, "good"), (2_300, 2_800, "idea")])];
    assert_eq!(texts(&two(a, sa, b, sb)), ["very very very very good idea"]);
}

#[test]
fn without_a_common_word_the_overlap_is_shared_at_its_middle() {
    let (a, b) = (region(0, 30_000), region(28_000, 58_000));
    let sa = vec![seg(&[(27_000, 27_500, "alpha"), (28_200, 28_700, "beta"), (29_300, 29_800, "gamm")])];
    let sb = vec![seg(&[(100, 600, "bet"), (1_300, 1_800, "gamma"), (2_200, 2_700, "delta")])];
    assert_eq!(texts(&two(a, sa, b, sb)), ["alpha beta gamma delta"]);
}

#[test]
fn a_side_that_heard_nothing_in_the_overlap_gives_way_to_the_other() {
    let (a, b) = (region(0, 30_000), region(28_000, 58_000));
    // The later window missed the overlap's words: the earlier one's stay.
    let sa = vec![seg(&[(27_000, 27_500, "one"), (28_500, 29_000, "two"), (29_400, 29_900, "three")])];
    let sb = vec![seg(&[(2_500, 3_000, "four")])];
    assert_eq!(texts(&two(a, sa, b, sb)), ["one two three", "four"]);
    // The earlier window missed them: the later one's are kept from the overlap's start.
    let sa = vec![seg(&[(25_000, 25_500, "one")])];
    let sb = vec![seg(&[(200, 700, "two"), (900, 1_400, "three"), (2_500, 3_000, "four")])];
    assert_eq!(texts(&two(a, sa, b, sb)), ["one", "two three four"]);
}

#[test]
fn segments_without_words_are_reconciled_as_whole_units() {
    let (a, b) = (region(0, 30_000), region(28_000, 58_000));
    let one = |a: i64, b: i64, t: &str| Segment { start_ms: a, end_ms: b, text: format!(" {t}"), words: Vec::new() };
    let sa = vec![one(20_000, 27_000, "first sentence."), one(27_500, 29_900, "Second sentence.")];
    let sb = vec![one(0, 1_800, "Second sentence."), one(2_000, 5_000, "Third sentence.")];
    assert_eq!(texts(&two(a, sa, b, sb)), ["first sentence.", "Second sentence.", "Third sentence."]);
}

#[test]
fn a_loop_on_a_sound_is_cut_and_said_repetition_and_numbers_stay() {
    for said in ["it is very very very good", "they just go chunk chunk chunk chunk chunk chunk chunk and", "in 1999 it cost 1000000000000 dollars"] {
        assert_eq!(collapse_repeats(said), said);
    }
    let spaced = format!("they just go{} and", " chon".repeat(100));
    assert_eq!(collapse_repeats(&spaced), format!("they just go{} and", " chon".repeat(8)));
    assert_eq!(collapse_repeats(&format!("go {}.", "chon".repeat(110))), format!("go {}.", "chon".repeat(8)));
    // Applied to what a window commits.
    let r = region(0, 10_000);
    let words: Vec<(i64, i64, &str)> = (0..40).map(|k| (k * 200, k * 200 + 150, "chon")).collect();
    let (cues, _) = window_cues(&r, vec![seg(&words)], Vec::new(), None, None, &mut 0);
    assert_eq!(texts(&cues), [vec!["chon"; 8].join(" ")]);
}
