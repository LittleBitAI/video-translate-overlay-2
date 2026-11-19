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
    }
    assert_eq!(sentences_of(&cues).len(), 3, "a request holds two sentences");
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
    assert_eq!(sentences_of(&cues), [comma], "the parts are not asked together");

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

/// The sentences translation asks for `cues` once saved ([`rebuilt_parts`]):
/// the texts of each sentence's parts joined by " ".
fn sentences_of(cues: &[Cue]) -> Vec<String> {
    let saved: Vec<crate::store::Cue> = cues.iter().enumerate().map(|(i, c)| crate::store::Cue { seq: i as i64, start_ms: c.start_ms, end_ms: c.end_ms, source_text: c.text.clone(), text_ko: None }).collect();
    let parts = rebuilt_parts(&saved);
    let mut out: Vec<String> = Vec::new();
    for (i, c) in cues.iter().enumerate() {
        match out.last_mut() {
            Some(s) if parts.contains(&(i as i64)) => *s = format!("{s} {}", c.text),
            _ => out.push(c.text.clone()),
        }
    }
    out
}

#[test]
fn a_punctuated_sentence_goes_whole_and_only_a_bare_stretch_ends_at_the_limit() {
    let clause = "the weights for that repeated block stay the same each time";
    let chained = format!("{clause}, {clause}, {clause}, {clause}, and we get a lot more depth going on there.");
    let long = "When we move from four layers to eight layers and then all the way up to a hundred layers in this looped model, we are honestly not yet sure whether there is going to be any real benefit at all or even a small cost.";
    let bare: Vec<String> = (0..60).map(|i| format!("word{i}")).collect();
    let bare = bare.join(" ");
    // Its first comma comes after the limit: the opening clause alone is longer.
    let opening = (0..40).map(|i| format!("step{i}")).collect::<Vec<_>>().join(" ");
    let late = format!("{opening}, and only then does the sentence end.");
    // The same past twice the limit, where the old look-ahead stopped.
    let opening = (0..70).map(|i| format!("step{i}")).collect::<Vec<_>>().join(" ");
    let later = format!("{opening}, and only then does the sentence end.");
    // Its only punctuation is its own end mark.
    let marked = (0..36).map(|i| format!("token{i}")).collect::<Vec<_>>().join(" ");
    let marked = format!("{marked}. Then a second one.");
    assert!(chained.len() > STRETCH_MAX && long.len() > STRETCH_MAX && bare.len() > STRETCH_MAX && late.len() > STRETCH_MAX);
    assert!(opening.len() > 2 * STRETCH_MAX && marked.len() > STRETCH_MAX + 20);
    // (form, the text, how many sentences it is)
    for (name, said, count) in [
        ("comma-chained run-on", chained.as_str(), 1),
        ("one long sentence, one comma", long, 1),
        ("first comma after the limit", late.as_str(), 1),
        ("first comma after twice the limit", later.as_str(), 1),
        ("only its end mark", marked.as_str(), 2),
        ("no punctuation, no pause", bare.as_str(), 2),
    ] {
        let cues = all(vec![owned(&spoken(0, 200, 50, said))]);
        let joined: Vec<&str> = cues.iter().flat_map(|c| c.text.split(' ')).collect();
        assert_eq!(joined, said.split(' ').collect::<Vec<_>>(), "{name}: every word once, in order");
        assert_eq!(sentences_of(&cues).len(), count, "{name}: {:?}", texts(&cues));
        assert!(cues.len() > count, "{name}: not cut into parts: {:?}", texts(&cues));
        for c in &cues {
            assert!(c.text.chars().count() <= STRETCH_MAX, "{name}: {c:?}");
        }
        ordered(&cues);
    }
}

#[test]
fn a_bare_stretch_before_a_long_pause_still_ends_at_the_limit() {
    let mut words = spoken(0, 200, 50, &(0..60).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" "));
    let after = words.last().unwrap().1 + 3_200;
    words.extend(spoken(after, 200, 50, "and then it ends."));
    let sentences = sentences_of(&all(vec![owned(&words)]));
    assert!(sentences.len() >= 3, "{sentences:?}");
    // Ended at the cue that reaches the limit.
    for s in &sentences {
        assert!(s.chars().count() <= STRETCH_MAX + CAP, "{s}");
    }
}

#[test]
fn no_request_holds_more_than_the_ceiling() {
    let clause = "the weights for that repeated block stay the same each time";
    let said = format!("{}.", vec![clause; 2 * CEILING / clause.len() + 2].join(", "));
    assert!(said.len() > 2 * CEILING);
    let cues = all(vec![owned(&spoken(0, 200, 50, &said))]);
    let sentences = sentences_of(&cues);
    assert!(sentences.len() >= 3, "{} sentences", sentences.len());
    for s in &sentences {
        assert!(s.chars().count() <= CEILING, "{} characters", s.chars().count());
    }
    assert_eq!(sentences.join(" "), said, "every word once, in order");
}

#[test]
fn a_sentence_written_across_windows_is_grouped_as_in_one() {
    let said = (0..70).map(|i| format!("step{i}")).collect::<Vec<_>>().join(" ");
    let said = format!("{said}, and only then does it end. Then {said}. Last.");
    let words = spoken(0, 200, 50, &said);
    let whole = all(vec![owned(&words)]);
    let (mut tail, mut prev_end, mut cues) = (Tail::new(), 0, Vec::new());
    let windows: Vec<&[(i64, i64, String)]> = words.chunks(17).collect();
    for (i, w) in windows.iter().enumerate() {
        cues.extend(push(&mut tail, vec![owned(w)], i + 1 == windows.len(), &mut prev_end));
    }
    assert_eq!(cues, whole);
    assert_eq!(sentences_of(&cues).len(), 3);
}

#[test]
fn grouping_an_hour_of_unpunctuated_speech_is_quick() {
    let saved: Vec<crate::store::Cue> = (0..600)
        .map(|i| crate::store::Cue { seq: i, start_ms: i * 1_100, end_ms: i * 1_100 + 1_000, source_text: (0..5).map(|k| format!("word{k}")).collect::<Vec<_>>().join(" "), text_ko: None })
        .collect();
    let started = std::time::Instant::now();
    rebuilt_parts(&saved);
    assert!(started.elapsed() < std::time::Duration::from_secs(1), "{:?}", started.elapsed());
}

#[test]
fn a_sentence_ends_where_its_punctuation_ends_it() {
    // (form, said, its sentences)
    for (name, said, sentences) in [
        ("question before lower case", "Any questions? we're done.", &["Any questions?", "we're done."][..]),
        ("exclamation before lower case", "Stop! now we start.", &["Stop!", "now we start."]),
        ("question before upper case", "Is it fast? Yes it is.", &["Is it fast?", "Yes it is."]),
        ("period before lower case", "The test passed. we're done.", &["The test passed.", "we're done."]),
        ("quoted question carried on", "He asked \"why?\" and left.", &["He asked \"why?\" and left."]),
        ("quoted exclamation ending", "She said \"stop!\" Then it ended.", &["She said \"stop!\"", "Then it ended."]),
        ("abbreviations", "We compare e.g. ResNet with Dr. Smith's model. Fine.", &["We compare e.g. ResNet with Dr. Smith's model.", "Fine."]),
        ("initialism before lower case", "Train on the U.S. data at 9 a.m. and stop.", &["Train on the U.S. data at 9 a.m. and stop."]),
        ("initialism before upper case", "It was made in the U.S. Then it shipped.", &["It was made in the U.S.", "Then it shipped."]),
        ("ellipsis before lower case", "so... and then it stops.", &["so... and then it stops."]),
        ("etc. before upper and lower case", "They ship GGUFs etc. So it runs locally, etc. and it is fast.", &["They ship GGUFs etc.", "So it runs locally, etc. and it is fast."]),
        ("ellipsis before a clause in lower case", "So... we start again.", &["So... we start again."]),
    ] {
        let cues = all(vec![owned(&spoken(0, 200, 50, said))]);
        assert_eq!(sentences_of(&cues), sentences, "{name}");
        ordered(&cues);
    }
}

#[test]
fn after_an_initialism_etc_or_an_ellipsis_a_lower_case_word_carries_the_sentence_on() {
    // Property: whatever lower-case word follows, and with or without a pause,
    // the sentence goes on; before upper case it ends.
    let marks = ["U.S.", "a.m.", "etc.", "so..."];
    let follows = ["this morning", "the next day", "these days", "we know is big", "the same way", "the software was written in Canada", "it ran well"];
    for (mark, follow) in marks.iter().flat_map(|m| follows.iter().map(move |f| (m, f))) {
        for pause in [50, 400, 1_200] {
            let mut words = spoken(0, 200, 50, &format!("It started at the {mark}"));
            let at = words.last().unwrap().1 + pause;
            words.extend(spoken(at, 200, 50, &format!("{follow}.")));
            let said = format!("It started at the {mark} {follow}.");
            assert_eq!(sentences_of(&all(vec![owned(&words)])), [said.clone()], "{said} after {pause} ms");
        }
        let said = format!("It started at the {mark} Then {follow}.");
        assert_eq!(sentences_of(&all(vec![owned(&spoken(0, 200, 50, &said))])), [format!("It started at the {mark}"), format!("Then {follow}.")], "{said}");
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
        // A sentence with no time right after an ellipsis that it ends: it stays
        // with the sentence before, so the saved cues still show that end.
        ("after an ellipsis", vec![at(&[(0, 500, "No...")]), at(&[(500, 500, "Okay.")]), at(&[(510, 800, "we"), (800, 1_100, "left.")])], &["No... Okay.", "we left."]),
        // The same with a word that overlaps the one before and ends with it.
        ("overlapping", vec![at(&[(0, 500, "No...")]), at(&[(450, 500, "Okay.")]), at(&[(510, 800, "we"), (800, 1_100, "left.")])], &["No... Okay.", "we left."]),
    ] {
        let last_word = pieces.iter().flat_map(|p| &p.words).map(|w| w.end_ms).max().unwrap();
        let said: Vec<String> = pieces.iter().flat_map(|p| &p.words).flat_map(|w| w.text.split_whitespace().map(String::from).collect::<Vec<_>>()).collect();
        let cues = all(pieces);
        assert_eq!(texts(&cues), sentences, "{name}");
        assert!(cues.iter().all(|c| c.end_ms <= last_word), "{name}: a cue ends after the last word: {cues:?}");
        if name != "final" {
            assert_eq!(cues.iter().flat_map(|c| c.text.split(' ')).collect::<Vec<_>>(), said, "{name}: a word was dropped");
        }
        ordered(&cues);
    }
}

#[test]
fn the_parts_of_a_sentence_are_asked_together_and_with_nothing_else() {
    let mut paused = spoken(0, 300, 100, "So the model keeps the full precision weights for the updates and rounds them on every forward pass. Done.");
    for w in paused.iter_mut().skip(9) {
        (w.0, w.1) = (w.0 + 1_600, w.1 + 1_600);
    }
    // (form, words, the sentences asked)
    for (name, words, sentences) in [
        (
            "comma",
            spoken(0, 250, 50, "When the learning rate is set far too high for this particular model, the training loss jumps around and never settles. Then we lower it."),
            &["When the learning rate is set far too high for this particular model, the training loss jumps around and never settles.", "Then we lower it."][..],
        ),
        (
            "conjunction",
            spoken(0, 250, 50, "The encoder reads the whole input sentence once and the decoder writes the output one token at a time. Next."),
            &["The encoder reads the whole input sentence once and the decoder writes the output one token at a time.", "Next."],
        ),
        ("pause", paused, &["So the model keeps the full precision weights for the updates and rounds them on every forward pass.", "Done."]),
    ] {
        let cues = all(vec![owned(&words)]);
        assert!(cues.len() > sentences.len(), "{name}: not cut into parts: {:?}", texts(&cues));
        assert_eq!(sentences_of(&cues), sentences, "{name}");
    }
}

#[test]
fn a_saved_cue_holding_a_sentence_end_closes_its_sentence() {
    // Cues written under earlier rules hold a sentence end before their last
    // word; after the comma, the sentence would otherwise run on to the end.
    let saved: Vec<Cue> = [
        "So if we wanted to detect a face,",
        "it first has to detect edges in the image. lines",
        "and edges and when you combine those lines",
        "you get corners and curves and then eyes and noses",
    ]
    .iter()
    .enumerate()
    .map(|(i, t)| Cue { start_ms: i as i64 * 3_000, end_ms: i as i64 * 3_000 + 2_900, text: t.to_string() })
    .collect();
    assert_eq!(
        sentences_of(&saved),
        [
            "So if we wanted to detect a face, it first has to detect edges in the image. lines",
            "and edges and when you combine those lines you get corners and curves and then eyes and noses"
        ]
    );
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
    assert_eq!(sentences_of(&cues).len(), 2, "two sentences, never translated together");
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
