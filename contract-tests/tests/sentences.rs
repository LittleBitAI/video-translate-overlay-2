//! Sentence cues in translation: a sentence split over cues goes to Gemini as
//! one request of its parts, mishearings ride with the glossary, and fillers
//! stay in the saved source while the instruction leaves them out of the Korean.

use serde_json::Value;
use vto_contract_tests::*;

const TALK: &str = "https://lectures.example.org/talks/42";

/// Longer than one cue, with a comma: written as two parts of one sentence.
const LONG: &str = "When the learning rate is set far too high for this particular model, the training loss jumps around and never settles.";
const SHORT: &str = "Then we lower it.";

fn translate_calls(app: &App) -> Vec<GeminiCall> {
    app.gemini.task_calls().into_iter().filter(|c| c.task.as_deref() == Some("translate")).collect()
}

fn segments(c: &GeminiCall) -> Vec<(i64, String)> {
    c.order.as_ref().unwrap()["segments"].as_array().unwrap().iter().map(|s| (s["id"].as_i64().unwrap(), s["text"].as_str().unwrap().to_string())).collect()
}

/// A talk of `lines`, one per spoken run of 6 s.
fn talk(lines: &[&str], gemini: GeminiOptions) -> (App, Vec<Value>) {
    let mut app = App::new(Options { lines: lines.iter().map(|l| l.to_string()).collect(), ms_per_audio_second: 20, gemini });
    let n = lines.len() as u64;
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(n * 8_000 + 1_000, &regular_spans(n, 500, 8_000, 6_000)));
    app.launch();
    app.set_key(KEY);
    let seen = app.run_job(TALK, Some(&url), "Learning rates", n * 8_000 + 1_000);
    let key = seen[0]["video_key"].as_str().unwrap().to_string();
    let cues = cues(&app.subtitles(TALK, &key));
    (app, cues)
}

#[test]
fn a_split_sentence_goes_to_gemini_as_one_request_of_its_parts() {
    let (app, cues) = talk(&[LONG, SHORT], GeminiOptions::default());
    assert_eq!(cues.len(), 3, "the long sentence is two cues, the short one one: {cues:?}");
    let calls = translate_calls(&app);
    assert_eq!(calls.len(), 2, "one request per sentence");
    let whole = segments(&calls.iter().find(|c| segments(c).len() == 2).expect("the split sentence went out in parts"));
    assert_eq!(whole.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [0, 1]);
    assert_eq!(whole.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(" "), LONG);
    // Each part's Korean is the reply's part with its id.
    for (cue, (_, text)) in cues.iter().zip(&whole) {
        assert_eq!(cue["text_ko"].as_str().unwrap(), format!("한국어 번역: {text}"));
    }
    // No request carries anything said in a later sentence.
    for c in &calls {
        let first = segments(c)[0].0;
        let order = c.order.as_ref().unwrap().to_string();
        if first == 0 {
            assert!(!order.contains("lower it"), "the first sentence's request holds the next one: {order}");
            assert!(c.order.as_ref().unwrap().get("context_before").is_none());
        } else {
            assert_eq!(c.order.as_ref().unwrap()["context_before"].as_str(), Some(LONG), "the sentence before, read-only");
        }
    }
}

#[test]
fn a_reply_with_a_missing_extra_or_duplicate_part_is_not_saved() {
    for fault in ["missing", "extra", "duplicate"] {
        let (app, cues) = talk(&[LONG, SHORT], GeminiOptions { parts_fault: Some(fault), ..Default::default() });
        for c in &cues {
            let ko = c["text_ko"].as_str().unwrap();
            assert!(has_hangul(ko) && !ko.contains("BROKEN"), "{fault}: a wrong reply was saved: {c}");
        }
        let calls = translate_calls(&app);
        let whole = calls.iter().filter(|c| segments(c).len() == 2).count();
        let single: Vec<i64> = calls.iter().filter(|c| segments(c).len() == 1).map(|c| segments(c)[0].0).collect();
        // Asked whole three times, then part by part; each part once, the other sentence once.
        assert_eq!(whole, 3, "{fault}");
        let mut single = single;
        single.sort();
        assert_eq!(single, [0, 1, 2], "{fault}");
    }
}

#[test]
fn mishearings_found_with_the_glossary_are_stored_and_carried_in_every_request() {
    let lines = ["The realu keeps positive values as they are.", "A realu sets the negative ones to zero.", "Attention comes next."];
    let (app, cues) = talk(&lines, GeminiOptions::default());
    assert_eq!(cues.len(), 3);
    assert!(app.sqlite_contains(br#"{"heard":"realu","meant":"ReLU"}"#), "the mishearing is not stored with the glossary");
    let calls = translate_calls(&app);
    assert_eq!(calls.len(), 3);
    for c in &calls {
        let m = &c.order.as_ref().unwrap()["mishearings"];
        assert_eq!(m, &serde_json::json!([{"heard": "realu", "meant": "ReLU"}]), "request without the mishearings: {:?}", c.order);
    }
}

#[test]
fn fillers_stay_in_the_saved_source_and_the_instruction_leaves_them_out_of_the_korean() {
    let said = "Uh, so the, the model, um, learns its weights, right?";
    let (app, _) = talk(&[said], GeminiOptions::default());
    assert!(app.sqlite_contains(said.as_bytes()), "the source was not saved as heard");
    let calls = translate_calls(&app);
    assert!(!calls.is_empty());
    for c in &calls {
        let sent: String = segments(c).into_iter().map(|(_, t)| t).collect::<Vec<_>>().join(" ");
        assert_eq!(sent, said, "the request's source differs from what was heard");
        for rule in ["Leave out of the Korean fillers, false starts, immediate repeats and verbal tics", "uh, um", "I mean", "right?", "add nothing that was not said"] {
            assert!(c.system.contains(rule), "the instruction lacks {rule:?}");
        }
    }
}
