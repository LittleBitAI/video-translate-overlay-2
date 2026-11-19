//! Cues rebuilt by sentence from the words a window commits.
//!
//! The speech model's segments end wherever its decoder stopped, often inside
//! a sentence and sometimes after two. Here the committed words are regrouped
//! so that each cue belongs to one sentence. A sentence longer than [`CAP`]
//! characters is cut into parts at a comma, before a conjunction, or at the
//! longest pause; a stretch with no punctuation is cut at pauses and the cap.
//! The parts of one sentence are marked so translation can take them in one
//! request (A6).
//!
//! The open sentence at the end of a window is held (and stored with the job)
//! until a later word ends it, so a resumed job cuts exactly as an
//! uninterrupted one.

use crate::stitch::Piece;
use crate::Word;

/// The most characters of English one cue holds. The phone fits about 56
/// Korean characters on two lines. In saved cues, 6 % of 70–80-character English
/// cues ran past that in Korean, against 2 % at 60–70.
pub const CAP: usize = 70;
/// A part shorter than this is not cut off on its own (except at a long pause).
const MIN_PART: usize = 20;
/// A pause this long always separates two parts of a sentence.
const PART_GAP_MS: i64 = 1_500;
/// A pause this long ends the sentence even without punctuation: content must
/// never be shown across a silence this long.
const SENTENCE_GAP_MS: i64 = 3_000;
/// An unpunctuated stretch past the cap ends at a pause this long.
const STRETCH_GAP_MS: i64 = 1_000;
/// A sentence ends here at the latest whatever its pauses: at its last comma
/// if it has one, so a run-on is not cut mid-clause.
const STRETCH_MAX: usize = 3 * CAP;

/// One cue: its time on the source clock, its text, and whether it opens a
/// sentence (false: it continues the cue before it).
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub opens: bool,
}

/// Words of the sentence not finished yet, carried from window to window.
pub type Tail = Vec<Word>;

/// Adds `pieces` (committed in order) to the open sentence `tail` and returns
/// the cues of every sentence that is now complete. `last`: no more words will
/// come, so the open sentence ends too. Cues start at or after `prev_end`.
pub fn push(tail: &mut Tail, pieces: Vec<Piece>, last: bool, prev_end: &mut i64) -> Vec<Cue> {
    tail.extend(pieces.into_iter().flat_map(words_of));
    let mut cues = Vec::new();
    let mut from = 0;
    let mut i = 0;
    while i < tail.len() {
        let next = tail.get(i + 1);
        let end = match next {
            None if !last => break, // whether it ends depends on the word after it
            None => true,
            Some(n) => {
                let gap = n.start_ms - tail[i].end_ms;
                let chars = len(&tail[from..=i]);
                ends_sentence(&tail[i], n) || gap >= SENTENCE_GAP_MS || (chars >= CAP && gap >= STRETCH_GAP_MS && !punctuated(&tail[from..=i]))
            }
        };
        let stop = if end {
            Some(i)
        } else if len(&tail[from..=i]) >= STRETCH_MAX {
            // A run-on past the limit ends at its last comma, not mid-clause.
            Some((from..i).rev().find(|&k| tail[k].text.trim_end().ends_with([',', ';', ':']) && len(&tail[from..=k]) >= CAP).unwrap_or(i))
        } else {
            None
        };
        if let Some(k) = stop {
            cues.extend(sentence_cues(&tail[from..=k], prev_end));
            from = k + 1;
        }
        i += 1;
    }
    tail.drain(..from);
    cues
}

/// A piece's words, the first starting with the piece and the last ending
/// with it. A word holding spaces (a segment the model gave without word
/// times) is spread over its time by character count.
fn words_of(p: Piece) -> Vec<Word> {
    let mut out = Vec::new();
    for w in p.words {
        let parts: Vec<&str> = w.text.split_whitespace().collect();
        if parts.len() <= 1 {
            out.push(w);
            continue;
        }
        let total: usize = parts.iter().map(|s| s.chars().count()).sum::<usize>().max(1);
        let span = (w.end_ms - w.start_ms).max(0);
        let mut done = 0;
        for s in parts {
            let n = s.chars().count();
            let a = w.start_ms + span * done as i64 / total as i64;
            done += n;
            let b = w.start_ms + span * done as i64 / total as i64;
            out.push(Word { start_ms: a, end_ms: b.max(a), text: format!(" {s}") });
        }
    }
    if let Some(f) = out.first_mut() {
        f.start_ms = p.start_ms.min(f.start_ms);
    }
    if let Some(l) = out.last_mut() {
        l.end_ms = p.end_ms.max(l.end_ms);
    }
    out
}

fn len(ws: &[Word]) -> usize {
    text(ws).chars().count()
}

fn text(ws: &[Word]) -> String {
    ws.iter().map(|w| w.text.as_str()).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether the stretch holds punctuation inside a sentence (a comma or the
/// like): then the model is punctuating, and only a sentence end ends it.
fn punctuated(ws: &[Word]) -> bool {
    ws.iter().any(|w| w.text.trim_end().ends_with([',', ';', ':']))
}

// Never followed by a new sentence. "etc." is not here: before a capital it
// ends one ("GGUFs etc. So the model…"), before lower case it carries on anyway.
const ABBREVIATIONS: [&str; 13] = ["mr.", "mrs.", "ms.", "dr.", "prof.", "jr.", "sr.", "vs.", "e.g.", "i.e.", "fig.", "eq.", "approx."];

/// Whether `w` ends a sentence, given the word `next` after it. A question or
/// exclamation mark always does. A period or an ellipsis does unless it ends
/// an abbreviation or the next word carries on in lower case ("U.S.
/// government", "e.g. this", "so... and").
fn ends_sentence(w: &Word, next: &Word) -> bool {
    let t = w.text.trim().trim_end_matches(['"', '\'', ')', ']', '”', '’']);
    if t.ends_with(['?', '!', '？', '！']) {
        return true;
    }
    if !t.ends_with(['.', '…', '。']) {
        return false;
    }
    let word = t.trim_start_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    if ABBREVIATIONS.contains(&word.as_str()) {
        return false;
    }
    !next.text.trim_start().trim_start_matches(|c: char| !c.is_alphanumeric()).starts_with(char::is_lowercase)
}

const CONJUNCTIONS: [&str; 19] = [
    "and", "but", "or", "so", "because", "when", "where", "while", "if", "although", "though", "then", "since", "until", "unless",
    "whereas", "whether", "after", "before",
];
/// A clause these open modifies the noun before it, and Korean puts it ahead
/// of that noun: a cut before one leaves the Korean of the first part ending
/// on the noun and the second on a dangling modifier ("…모델과 다릅니다 /
/// 여러분이 아는.").
const RELATIVES: [&str; 5] = ["that", "which", "who", "whose", "whom"];

/// How good a place the boundary after `ws[k]` is: after a comma (3), before
/// a conjunction (2), anywhere else (1), before a relative pronoun (0).
fn kind(ws: &[Word], k: usize) -> u8 {
    if ws[k].text.trim_end().ends_with([',', ';', ':', '—', '–']) {
        return 3;
    }
    let next: String = ws[k + 1].text.trim().trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    if CONJUNCTIONS.contains(&next.as_str()) {
        2
    } else if RELATIVES.contains(&next.as_str()) {
        0
    } else {
        1
    }
}

fn gap(ws: &[Word], k: usize) -> i64 {
    ws[k + 1].start_ms - ws[k].end_ms
}

/// The parts of one sentence, as ranges of its words.
fn parts(ws: &[Word]) -> Vec<std::ops::Range<usize>> {
    // A long pause always separates parts; each side is then cut on its own.
    let mut out = Vec::new();
    let mut from = 0;
    for k in 0..ws.len().saturating_sub(1) {
        if gap(ws, k) >= PART_GAP_MS {
            cut(ws, from..k + 1, &mut out);
            from = k + 1;
        }
    }
    cut(ws, from..ws.len(), &mut out);
    out
}

/// Cuts `r` until every part fits the cap: at the comma nearest its middle,
/// or else before the conjunction nearest its middle, or else at its longest
/// pause, keeping both sides at least [`MIN_PART`] long where it can.
fn cut(ws: &[Word], r: std::ops::Range<usize>, out: &mut Vec<std::ops::Range<usize>>) {
    let all = len(&ws[r.clone()]);
    if all <= CAP || r.len() < 2 {
        out.push(r);
        return;
    }
    let half = all as i64 / 2;
    let candidates: Vec<(usize, usize)> = (r.start..r.end - 1).map(|k| (k, len(&ws[r.start..=k]))).collect();
    let fair: Vec<(usize, usize)> = candidates.iter().copied().filter(|&(_, l)| l >= MIN_PART && all - l >= MIN_PART).collect();
    let pool = if fair.is_empty() { &candidates } else { &fair };
    let best = pool
        .iter()
        .max_by_key(|&&(k, l)| {
            let kind = kind(ws, k);
            // Commas and conjunctions: the most even cut; elsewhere: the longest pause, then the most even.
            let pause = if kind == 1 { gap(ws, k) } else { 0 };
            (kind, pause, -(l as i64 - half).abs())
        })
        .unwrap()
        .0;
    cut(ws, r.start..best + 1, out);
    cut(ws, best + 1..r.end, out);
}

/// The cues of one sentence, kept in order after `prev_end`. A cue never ends
/// after its last word, so it stays inside the audio the words came from: a
/// part whose words take no time gets the millisecond before them. A part
/// left no time of its own (its words end at or before `prev_end`) joins the
/// part before it; a whole sentence left no time is dropped, as the segment
/// cues before sentence cues dropped such a segment.
fn sentence_cues(ws: &[Word], prev_end: &mut i64) -> Vec<Cue> {
    let mut cues: Vec<Cue> = Vec::new();
    for r in parts(ws) {
        let part = &ws[r];
        let text = text(part);
        if text.is_empty() {
            continue;
        }
        let end = part.last().unwrap().end_ms;
        let mut start = part[0].start_ms.max(*prev_end);
        if end <= start && end > *prev_end {
            start = end - 1;
        }
        if end <= start {
            if let Some(before) = cues.last_mut() {
                before.text = format!("{} {text}", before.text);
            }
            continue;
        }
        *prev_end = end;
        cues.push(Cue { start_ms: start, end_ms: end, text, opens: cues.is_empty() });
    }
    cues
}

/// Which of the saved `cues` (in seq order) continue the sentence of the cue
/// before: the grouping [`push`] made, rebuilt from their text and times where
/// it was not kept (a saved version projected or received from another
/// device, a re-translation of one). A cue opens a sentence after a sentence
/// end, a pause of [`SENTENCE_GAP_MS`], an unpunctuated stretch past the cap
/// at a pause of [`STRETCH_GAP_MS`], or once the sentence has reached
/// [`STRETCH_MAX`]; these are the ends `push` makes, except that `push` may end
/// a run-on earlier, at its last comma.
pub fn rebuilt_parts(cues: &[crate::store::Cue]) -> std::collections::HashSet<i64> {
    let mut out = std::collections::HashSet::new();
    let mut sentence: Vec<Word> = Vec::new();
    for (i, c) in cues.iter().enumerate() {
        let words: Vec<Word> = c.source_text.split_whitespace().map(|t| Word { start_ms: c.start_ms, end_ms: c.end_ms, text: format!(" {t}") }).collect();
        let Some(first) = words.first() else {
            sentence.clear();
            continue;
        };
        let continues = i > 0 && cues[i - 1].seq + 1 == c.seq && sentence.last().is_some_and(|last| {
            let (gap, chars) = (c.start_ms - cues[i - 1].end_ms, len(&sentence));
            !ends_sentence(last, first) && gap < SENTENCE_GAP_MS && !(chars >= CAP && gap >= STRETCH_GAP_MS && !punctuated(&sentence)) && chars < STRETCH_MAX
        });
        if continues {
            out.insert(c.seq);
        } else {
            sentence.clear();
        }
        sentence.extend(words);
    }
    out
}

#[cfg(test)]
#[path = "sentences_tests.rs"]
mod tests;
