//! The cues of one written window on the source timeline, and the boundary
//! between two windows that overlap because speech crossed the cut there.
//!
//! Every word is committed once. A window that the next one overlaps holds
//! back its cues reaching into the overlap. The next window's head is then
//! reconciled with them by matching words (text, and time within a tolerance):
//! a word both windows heard is kept from one of them, a word only one heard is
//! kept from that one, and a cue the cut split becomes one cue again.

use crate::audio::Region;
use crate::{Segment, Word};
use serde::{Deserialize, Serialize};

/// A cue on the source timeline with its words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Piece {
    pub start_ms: i64,
    pub end_ms: i64,
    pub words: Vec<Word>,
}

impl Piece {
    pub fn text(&self) -> String {
        self.words.iter().map(|w| w.text.as_str()).collect::<String>().trim().to_string()
    }
}

/// How far outside the overlap a word may lie and still be matched.
const SLACK_MS: i64 = 1_000;
/// How far apart in time two heard words may be and still be the same word:
/// a single word must agree more closely than a run of them.
const RUN_TOLERANCE_MS: i64 = 1_000;
const WORD_TOLERANCE_MS: i64 = 500;

/// What window `r` adds: the cues to commit now, and those held for `next`
/// when it overlaps `r`. `held` are the previous window's held cues, `prev`
/// that window; the cues committed keep starting at or after `prev_end`.
pub fn window_cues(
    r: &Region,
    segs: Vec<Segment>,
    held: Vec<Piece>,
    prev: Option<&Region>,
    next: Option<&Region>,
    prev_end: &mut i64,
) -> (Vec<(i64, i64, String)>, Vec<Piece>) {
    let mut ps = pieces(r, segs);
    if !held.is_empty() {
        let shared_until = prev.map_or(r.start_ms(), |p| p.end_ms()).max(r.start_ms());
        ps = join(held, ps, r.start_ms(), shared_until);
    }
    let held = match next.filter(|n| n.start < r.end) {
        Some(n) => {
            let from = ps.iter().position(|p| p.end_ms > n.start_ms()).unwrap_or(ps.len());
            ps.split_off(from)
        }
        None => Vec::new(),
    };
    let mut cues = Vec::new();
    for p in ps {
        let text = p.text();
        let start = p.start_ms.max(*prev_end);
        if text.is_empty() || p.end_ms <= start {
            continue;
        }
        *prev_end = p.end_ms;
        cues.push((start, p.end_ms, text));
    }
    (cues, held)
}

/// The segments of window `r` on the source timeline, kept inside it.
fn pieces(r: &Region, segs: Vec<Segment>) -> Vec<Piece> {
    let (a, b) = (r.start_ms(), r.end_ms());
    segs.into_iter()
        .filter_map(|s| {
            let (start, end) = ((a + s.start_ms).clamp(a, b), (a + s.end_ms).clamp(a, b));
            let words = if s.words.is_empty() {
                vec![Word { start_ms: start, end_ms: end, text: s.text }]
            } else {
                s.words
                    .into_iter()
                    .map(|w| Word { start_ms: (a + w.start_ms).clamp(start, end), end_ms: (a + w.end_ms).clamp(start, end), text: w.text })
                    .collect()
            };
            let p = Piece { start_ms: start, end_ms: end, words };
            (end > start && !p.text().is_empty()).then_some(p)
        })
        .collect()
}

fn mid(w: &Word) -> i64 {
    (w.start_ms + w.end_ms) / 2
}

/// A word as matched: letters and digits only, lower case.
fn key(w: &Word) -> String {
    w.text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// `a` (held from the earlier window) and `b` (the later window's cues), which
/// both heard `from..until`, as one sequence in which every word appears once.
fn join(a: Vec<Piece>, b: Vec<Piece>, from: i64, until: i64) -> Vec<Piece> {
    let aw: Vec<&Word> = a.iter().flat_map(|p| &p.words).collect();
    let bw: Vec<&Word> = b.iter().flat_map(|p| &p.words).collect();
    let middle = (from + until) / 2;
    // The words each side heard in and near the overlap.
    let a0 = aw.iter().position(|w| mid(w) >= from - SLACK_MS).unwrap_or(aw.len());
    let b1 = bw.iter().rposition(|w| mid(w) <= until + SLACK_MS).map_or(0, |i| i + 1);
    let same = |i: usize, j: usize| {
        let k = key(aw[i]);
        !k.is_empty() && k == key(bw[j]) && (mid(aw[i]) - mid(bw[j])).abs() <= RUN_TOLERANCE_MS
    };
    // The longest run both heard; of equally long ones (a word said again and
    // again), the one whose times agree best, then the one nearer the middle.
    let mut run = vec![vec![(0usize, 0i64); b1 + 1]; aw.len() + 1]; // (length, summed time apart)
    let mut best: Option<(usize, i64, usize, usize)> = None; // (length, time apart, a end, b end), exclusive ends
    for i in a0..aw.len() {
        for j in 0..b1 {
            if same(i, j) {
                let (n, apart) = run[i][j];
                let here = (n + 1, apart + (mid(aw[i]) - mid(bw[j])).abs());
                run[i + 1][j + 1] = here;
                let near = |e: usize| (mid(aw[e - 1]) - middle).abs();
                let better = best.is_none_or(|(m, d, e, _)| (here.0, -here.1, -near(i + 1)) > (m, -d, -near(e)));
                if better {
                    best = Some((here.0, here.1, i + 1, j + 1));
                }
            }
        }
    }
    let best = best.filter(|&(n, apart, _, _)| n >= 2 || apart <= WORD_TOLERANCE_MS);
    let (keep_a, keep_b): (Vec<bool>, Vec<bool>) = match best {
        Some((n, _, i, j)) => {
            // Change sides at the matched word nearest the middle of the overlap.
            let k = (0..n).min_by_key(|&k| (mid(aw[i - n + k]) - middle).abs()).unwrap();
            let (ia, ib) = (i - n + k, j - n + k);
            ((0..aw.len()).map(|x| x <= ia).collect(), (0..bw.len()).map(|x| x > ib).collect())
        }
        None => {
            // No word in common: change sides by time, at the middle, or at an
            // edge of the overlap when one side heard nothing in it.
            let heard = |ws: &[&Word]| ws.iter().any(|w| (from..=until).contains(&mid(w)));
            let at = if !heard(&aw) { from } else if !heard(&bw) { until } else { middle };
            (aw.iter().map(|w| mid(w) < at).collect(), bw.iter().map(|w| mid(w) >= at).collect())
        }
    };
    let (mut out, a_cut) = keep(a, &keep_a);
    let (later, b_cut) = keep(b, &keep_b);
    let mut later = later.into_iter();
    // A cue the cut split in two: its two halves become one cue again.
    if let (Some(true), Some(first)) = (a_cut.last(), later.clone().next()) {
        if b_cut.first() == Some(&true) {
            later.next();
            let last = out.last_mut().unwrap();
            last.end_ms = first.end_ms;
            last.words.extend(first.words);
        }
    }
    out.extend(later);
    out
}

/// The pieces with only the words `keep` marks (in order over all their
/// words); for each piece kept, whether it lost words at its end (for the
/// earlier side) or start (the later side) — either way, whether it was cut.
fn keep(ps: Vec<Piece>, keep: &[bool]) -> (Vec<Piece>, Vec<bool>) {
    let (mut out, mut cut, mut k) = (Vec::new(), Vec::new(), 0);
    for p in ps {
        let n = p.words.len();
        let flags = &keep[k..k + n];
        k += n;
        let words: Vec<Word> = p.words.into_iter().zip(flags).filter(|(_, &f)| f).map(|(w, _)| w).collect();
        if words.is_empty() {
            continue;
        }
        let start = if flags[0] { p.start_ms } else { words[0].start_ms };
        let end = if flags[n - 1] { p.end_ms } else { words.last().unwrap().end_ms.max(start + 1) };
        cut.push(words.len() < n);
        out.push(Piece { start_ms: start, end_ms: end, words });
    }
    (out, cut)
}

#[cfg(test)]
#[path = "stitch_tests.rs"]
mod tests;
