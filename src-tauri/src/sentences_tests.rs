//! Sentence cues from word-timed pieces.

use super::*;

/// A piece of words given as (start, end, text) on the source clock.
fn piece(words: &[(i64, i64, &str)]) -> Piece {
    let words: Vec<Word> = words.iter().map(|&(a, b, t)| Word { start_ms: a, end_ms: b, text: format!(" {t}") }).collect();
    Piece { start_ms: words[0].start_ms, end_ms: words.last().unwrap().end_ms, words }
}

/// Words of `text` one after another from `at`, each `step` ms with `gap` ms between.
fn spoken(at: i64, step: i64, gap: i64, text: &str) -> Vec<(i64, i64, String)> {
    text.split(' ').enumerate().map(|(i, w)| (at + i as i64 * (step + gap), at + i as i64 * (step + gap) + step, w.to_string())).collect()
}

fn owned(ws: &[(i64, i64, String)]) -> Piece {
    piece(&ws.iter().map(|(a, b, t)| (*a, *b, t.as_str())).collect::<Vec<_>>())
}

fn all(pieces: Vec<Piece>) -> Vec<Cue> {
    let (mut tail, mut prev_end) = (Tail::new(), 0);
    let cues = push(&mut tail, pieces, true, &mut prev_end);
    assert!(tail.is_empty());
    cues
}

fn texts(cues: &[Cue]) -> Vec<&str> {
    cues.iter().map(|c| c.text.as_str()).collect()
}

/// Sentence ends: `.`, `?` or `!` at the end of a cue's text.
fn ends(text: &str) -> usize {
    text.split(' ').filter(|w| w.ends_with(['.', '?', '!'])).count()
}

fn ordered(cues: &[Cue]) {
    for c in cues {
        assert!(c.start_ms < c.end_ms, "empty or reversed cue {c:?}");
    }
    for w in cues.windows(2) {
        assert!(w[0].end_ms <= w[1].start_ms, "cues overlap or are out of order: {w:?}");
    }
}

#[test]
fn each_cue_is_one_sentence_and_none_holds_the_end_of_one_and_the_start_of_the_next() {
    // The model's segments cut across sentences: one ends inside the first, the next holds two ends.
    let words = spoken(1_000, 300, 100, "Today we train a network. It learns from data. Then we test it on new images.");
    let cues = all(vec![owned(&words[..3]), owned(&words[3..12]), owned(&words[12..])]);
    assert_eq!(texts(&cues), ["Today we train a network.", "It learns from data.", "Then we test it on new images."]);
    for c in &cues {
        assert_eq!(ends(&c.text), 1, "{c:?}");
        assert!(c.opens);
    }
    ordered(&cues);
}

#[test]
fn a_sentence_starting_mid_segment_starts_with_its_first_word() {
    // One segment of 0-9 s; "Gradients" is spoken at 6,240 ms.
    let p = piece(&[(0, 900, "That"), (1_000, 1_800, "is"), (1_900, 3_000, "backprop."), (6_240, 6_900, "Gradients"), (7_000, 7_500, "flow"), (7_600, 9_000, "back.")]);
    let p = Piece { start_ms: 0, end_ms: 9_000, ..p };
    let cues = all(vec![p]);
    assert_eq!(texts(&cues), ["That is backprop.", "Gradients flow back."]);
    assert!((cues[1].start_ms - 6_240).abs() <= 300, "starts at {}", cues[1].start_ms);
    assert_eq!(cues[1].end_ms, 9_000);
    ordered(&cues);
}

#[test]
fn a_sentence_over_the_cap_is_cut_at_a_comma_then_a_conjunction_then_the_longest_pause() {
    let comma = "When the learning rate is too high for this model, the loss jumps around and never settles down at all.";
    let cues = all(vec![owned(&spoken(0, 200, 50, comma))]);
    assert_eq!(texts(&cues), ["When the learning rate is too high for this model,", "the loss jumps around and never settles down at all."]);
    assert!(cues[0].opens && !cues[1].opens);

    let conjunction = "The encoder reads the whole input sequence at once in parallel because attention needs no recurrence at any step.";
    let cues = all(vec![owned(&spoken(0, 200, 50, conjunction))]);
    assert_eq!(texts(&cues), ["The encoder reads the whole input sequence at once in parallel", "because attention needs no recurrence at any step."]);

    // Never before a relative pronoun, even at the longest pause: Korean puts the clause ahead of its noun.
    let mut ws = spoken(0, 200, 50, "This is unlike the many other large language models that you might already be familiar with.");
    for w in ws.iter_mut().skip(9) {
        w.0 += 700;
        w.1 += 700;
    }
    let cues = all(vec![owned(&ws)]);
    assert_eq!(cues.len(), 2, "{:?}", texts(&cues));
    assert!(!cues[1].text.starts_with("that"), "{:?}", texts(&cues));

    // No comma or conjunction: the longest pause, here after "weights".
    let mut ws = spoken(0, 200, 50, "Every single layer of the network stores its own weights in memory on the graphics card itself.");
    for w in ws.iter_mut().skip(10) {
        w.0 += 700;
        w.1 += 700;
    }
    let cues = all(vec![owned(&ws)]);
    assert_eq!(texts(&cues), ["Every single layer of the network stores its own weights", "in memory on the graphics card itself."]);
    for c in &cues {
        assert!(c.text.chars().count() <= CAP, "{c:?}");
    }
    ordered(&cues);
}

#[test]
fn a_stretch_without_punctuation_is_cut_at_pauses_and_the_cap() {
    // 60 words, no punctuation, a 1.2 s pause after word 20 and short gaps otherwise.
    let text: Vec<String> = (0..60).map(|i| format!("word{i}")).collect();
    let mut ws = spoken(0, 250, 80, &text.join(" "));
    for w in ws.iter_mut().skip(21) {
        w.0 += 1_200;
        w.1 += 1_200;
    }
    let cues = all(vec![owned(&ws)]);
    let joined: Vec<&str> = cues.iter().flat_map(|c| c.text.split(' ')).collect();
    assert_eq!(joined, text.iter().map(String::as_str).collect::<Vec<_>>(), "every word once, in order");
    for c in &cues {
        assert!(c.text.chars().count() <= CAP, "{c:?}");
    }
    assert!(cues.iter().any(|c| c.text.ends_with("word20")), "not cut at the pause: {:?}", texts(&cues));
    ordered(&cues);
}

#[test]
fn a_run_on_sentence_past_the_limit_ends_at_its_last_comma() {
    // Clauses joined by commas, no sentence end, no pause: over three caps long.
    let clause = "the weights for that repeated block stay the same each time";
    let text = format!("{}, {}, {}, {}, and we get a lot more depth going on there.", clause, clause, clause, clause);
    let cues = all(vec![owned(&spoken(0, 200, 50, &text))]);
    let joined: Vec<&str> = cues.iter().flat_map(|c| c.text.split(' ')).collect();
    assert_eq!(joined, text.split(' ').collect::<Vec<_>>(), "every word once, in order");
    // Where a sentence was forced to end, it ends after a comma, never mid-clause.
    for w in cues.windows(2).filter(|w| w[1].opens) {
        assert!(w[0].text.ends_with(','), "a sentence cut mid-clause: {:?}", texts(&cues));
    }
    assert!(cues.iter().skip(1).any(|c| c.opens), "the run-on was not ended at the limit: {:?}", texts(&cues));
    assert!(!cues.iter().any(|c| c.text == "on there."), "{:?}", texts(&cues));
    ordered(&cues);
}

#[test]
fn abbreviations_and_lower_case_continuations_do_not_end_a_sentence() {
    let cues = all(vec![owned(&spoken(0, 200, 50, "We compare e.g. ResNet with Dr. Smith's model on the U.S. dataset. Fine."))]);
    assert_eq!(texts(&cues), ["We compare e.g. ResNet with Dr. Smith's model on the U.S. dataset.", "Fine."]);
    // "etc." ends a sentence before a capital and carries on before lower case.
    let cues = all(vec![owned(&spoken(0, 200, 50, "They ship GGUFs etc. So it runs locally, etc. and it is fast."))]);
    assert_eq!(texts(&cues), ["They ship GGUFs etc.", "So it runs locally, etc. and it is fast."]);
}

#[test]
fn a_question_or_exclamation_mark_ends_a_sentence_whatever_follows() {
    for (said, sentences) in [
        ("Any questions? we're done.", &["Any questions?", "we're done."][..]),
        ("Stop! now we start.", &["Stop!", "now we start."]),
        ("Is it fast? Yes it is.", &["Is it fast?", "Yes it is."]),
        ("He asked \"why?\" and left.", &["He asked \"why?\"", "and left."]),
        // A period before lower case still carries on.
        ("Train on the U.S. data now.", &["Train on the U.S. data now."]),
        ("so... and then it stops.", &["so... and then it stops."]),
    ] {
        let cues = all(vec![owned(&spoken(0, 200, 50, said))]);
        assert_eq!(texts(&cues), sentences, "{said}");
        assert!(cues.iter().all(|c| c.opens), "{said}: each is its own sentence");
        ordered(&cues);
    }
}

#[test]
fn no_cue_ends_after_its_words_even_when_they_take_no_time() {
    let at = |ws: &[(i64, i64, &str)]| piece(ws);
    for (name, pieces, sentences) in [
        // A sentence ends at the audio's end, and the model writes one more there with no time.
        ("final", vec![at(&[(8_000, 9_000, "It"), (9_000, 10_000, "ends.")]), at(&[(10_000, 10_000, "Thanks.")])], &["It ends."][..]),
        // A one-word sentence with no time, between two others.
        ("between", vec![at(&[(0, 3_000, "First.")]), at(&[(5_000, 5_000, "Okay.")]), at(&[(6_000, 7_000, "Next.")])], &["First.", "Okay.", "Next."]),
        // The second part of a long sentence takes no time after the first: it joins it.
        (
            "part",
            vec![at(&[(0, 2_000, "When the learning rate is set far too high for this"), (2_000, 5_000, "particular model,")]), at(&[(5_000, 5_000, "the loss jumps around and never settles at all.")])],
            &["When the learning rate is set far too high for this particular model, the loss jumps around and never settles at all."],
        ),
    ] {
        let last_word = pieces.iter().flat_map(|p| &p.words).map(|w| w.end_ms).max().unwrap();
        let cues = all(pieces);
        assert_eq!(texts(&cues), sentences, "{name}");
        assert!(cues.iter().all(|c| c.end_ms <= last_word), "{name}: a cue ends after the last word: {cues:?}");
        ordered(&cues);
    }
}

#[test]
fn the_grouping_rebuilt_from_saved_cues_is_the_one_the_cues_were_made_with() {
    let mut silence = spoken(0, 200, 50, "thank you all for coming welcome to the course");
    for w in silence.iter_mut().skip(5) {
        (w.0, w.1) = (w.0 + 20_000, w.1 + 20_000);
    }
    let mut paused = spoken(0, 300, 100, "So the model keeps the full precision weights for the updates and rounds them on every forward pass. Done.");
    for w in paused.iter_mut().skip(9) {
        (w.0, w.1) = (w.0 + 1_600, w.1 + 1_600);
    }
    for (name, words) in [
        ("sentences", spoken(1_000, 300, 100, "Today we train a network. It learns from data. Then we test it on new images.")),
        ("comma", spoken(0, 250, 50, "When the learning rate is set far too high for this particular model, the training loss jumps around and never settles. Then we lower it.")),
        ("conjunction", spoken(0, 250, 50, "The encoder reads the whole input sentence once and the decoder writes the output one token at a time. Next.")),
        ("questions", spoken(0, 200, 50, "Any questions? we're done. Is it fast? Yes it is.")),
        ("unpunctuated", spoken(0, 250, 1_100, "so here the loss goes down and here it goes up again and then it settles somewhere in the middle of the range")),
        ("silence", silence),
        ("pause", paused),
    ] {
        let made = all(vec![owned(&words)]);
        let saved: Vec<crate::store::Cue> = made.iter().enumerate().map(|(i, c)| crate::store::Cue { seq: i as i64, start_ms: c.start_ms, end_ms: c.end_ms, source_text: c.text.clone(), text_ko: None }).collect();
        let parts: std::collections::HashSet<i64> = made.iter().enumerate().filter(|(_, c)| !c.opens).map(|(i, _)| i as i64).collect();
        assert_eq!(rebuilt_parts(&saved), parts, "{name}: {:?}", texts(&made));
    }
}

#[test]
fn a_long_silence_ends_a_sentence_without_punctuation() {
    let mut ws = spoken(0, 200, 50, "thank you all for coming welcome to the course");
    for w in ws.iter_mut().skip(5) {
        w.0 += 20_000;
        w.1 += 20_000;
    }
    let cues = all(vec![owned(&ws)]);
    assert_eq!(texts(&cues), ["thank you all for coming", "welcome to the course"]);
    assert!(cues.iter().all(|c| c.opens), "two sentences, never translated together");
}

#[test]
fn the_open_sentence_waits_for_the_next_window_and_ends_the_same() {
    let words = spoken(0, 200, 50, "First one is short. The second one runs across the cut between two windows. Last.");
    let cut = 7; // inside the second sentence
    let whole = all(vec![owned(&words)]);

    let (mut tail, mut prev_end) = (Tail::new(), 0);
    let mut cues = push(&mut tail, vec![owned(&words[..cut])], false, &mut prev_end);
    assert_eq!(texts(&cues), ["First one is short."]);
    // Through JSON, as the job keeps it between windows.
    let mut tail: Tail = serde_json::from_str(&serde_json::to_string(&tail).unwrap()).unwrap();
    cues.extend(push(&mut tail, vec![owned(&words[cut..])], true, &mut prev_end));
    assert_eq!(cues, whole);
}

#[test]
fn cues_keep_after_the_previous_one() {
    let (mut tail, mut prev_end) = (Tail::new(), 10_500);
    let p = piece(&[(10_000, 10_800, "Early"), (10_900, 12_000, "start.")]);
    let cues = push(&mut tail, vec![p], true, &mut prev_end);
    assert_eq!((cues[0].start_ms, cues[0].end_ms), (10_500, 12_000));
    assert_eq!(prev_end, 12_000);
}

#[test]
fn a_segment_without_word_times_is_spread_over_its_time() {
    let p = Piece { start_ms: 0, end_ms: 10_000, words: vec![Word { start_ms: 0, end_ms: 10_000, text: " One two. Three four five six.".into() }] };
    let cues = all(vec![p]);
    assert_eq!(texts(&cues), ["One two.", "Three four five six."]);
    assert!(cues[1].start_ms > 2_000 && cues[1].start_ms < 4_500, "{cues:?}");
    assert_eq!(cues[1].end_ms, 10_000);
}
