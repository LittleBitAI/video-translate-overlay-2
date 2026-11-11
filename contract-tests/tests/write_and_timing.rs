//! Writing speech on the device and keeping cue times on the source timeline.

use vto_contract_tests::*;

const TALK: &str = "https://lectures.example.org/talks/42";

#[test]
fn test_silent_span_has_no_cue() {
    let mut app = App::new(Options::default());
    // Speech for the first 10 s, then 20 s of silence.
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(30_000, &[(500, 3_000), (3_500, 6_500), (7_000, 9_800)]));
    app.launch();
    app.set_key(KEY);
    let seen = app.run_job(TALK, Some(&url), "Ten seconds of talk", 30_000);
    assert!(stages(&seen).iter().any(|s| s == "writing"), "never went through writing: {:?}", stages(&seen));

    let video_key = seen[0]["video_key"].as_str().unwrap().to_string();
    let cues = cues(&app.subtitles(TALK, &video_key));
    assert!(!cues.is_empty(), "the spoken part made no cue");
    let late: Vec<_> = cues.iter().filter(|c| ms(c, "start_ms") >= 10_000).collect();
    assert!(late.is_empty(), "cues inside the silence: {late:?}");
}

#[test]
fn test_cue_times_match_source_after_silence_trim() {
    let mut app = App::new(Options::default());
    // Long silences between short talks; the last sentence starts at exactly 80.000 s.
    let spans = [(1_000, 3_000), (20_000, 22_000), (45_000, 47_000), (80_000, 83_000)];
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(90_000, &spans));
    app.launch();
    app.set_key(KEY);
    let seen = app.run_job(TALK, Some(&url), "Talk with pauses", 90_000);
    let video_key = seen[0]["video_key"].as_str().unwrap().to_string();
    let cues = cues(&app.subtitles(TALK, &video_key));

    for c in &cues {
        assert!(ms(c, "start_ms") < ms(c, "end_ms"), "empty or reversed cue {c}");
    }
    for w in cues.windows(2) {
        assert!(ms(&w[0], "start_ms") <= ms(&w[1], "start_ms"), "cues out of order: {} then {}", w[0], w[1]);
        assert!(ms(&w[0], "end_ms") <= ms(&w[1], "start_ms"), "cues overlap: {} and {}", w[0], w[1]);
    }
    let last = cues
        .iter()
        .filter(|c| ms(c, "start_ms") >= 60_000)
        .map(|c| ms(c, "start_ms"))
        .min()
        .unwrap_or_else(|| panic!("no cue for the sentence at 80 s: {cues:?}"));
    assert!((last - 80_000).abs() <= 300, "the sentence spoken at 80000 ms shows at {last} ms");
}
