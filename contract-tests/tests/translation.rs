//! Translation with Gemini: the model used, the cost, the cost limit and the glossary.

use vto_contract_tests::*;

const TALK: &str = "https://lectures.example.org/talks/42";

#[test]
fn test_translation_makes_korean_cues() {
    let mut app = App::new(Options { gemini: GeminiOptions { delay_ms: 300, ..Default::default() }, ..Options::default() });
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(12_000, &regular_spans(4, 500, 3_000, 2_000)));
    app.launch();
    app.set_key(KEY);
    let seen = app.run_job(TALK, Some(&url), "Transformers in twelve seconds", 12_000);
    let order = stages(&seen);
    let t = order.iter().position(|s| s == "translating");
    let r = order.iter().position(|s| s == "ready");
    assert!(matches!((t, r), (Some(t), Some(r)) if t < r), "not translating then ready: {order:?}");

    let video_key = seen[0]["video_key"].as_str().unwrap().to_string();
    let cues = cues(&app.subtitles(TALK, &video_key));
    assert!(!cues.is_empty());
    for c in &cues {
        let ko = c["text_ko"].as_str().unwrap_or("");
        assert!(has_hangul(ko), "cue without Korean text: {c}");
    }
    let calls = app.gemini.generate_calls();
    assert!(calls.iter().any(|c| c.task.as_deref() == Some("translate")), "no translate request reached Gemini");
    for c in &calls {
        assert_eq!(c.model.as_deref(), Some(MODEL), "Gemini was asked for another model: {}", c.path);
    }
}

#[test]
fn test_one_hour_lecture_costs_under_budget() {
    let lines: Vec<String> = (0..10)
        .map(|i| {
            format!(
                "Part {i}: in this section we discuss how attention heads in a transformer share work, why layer \
                 normalisation keeps training stable, how the learning rate schedule interacts with batch size, and \
                 what the scaling laws say about compute-optimal model sizes for a fixed budget of tokens."
            )
        })
        .collect();
    let mut app = App::new(Options { lines, ms_per_audio_second: 5, ..Options::default() });
    let hour = 3_600_000;
    let url = app.media.serve("/talks/hour.wav", "audio/wav", speech_wav(hour, &regular_spans(170, 500, 21_000, 1_500)));
    app.launch();
    app.set_key(KEY);
    let before = app.gemini.task_calls().len();
    app.run_job(TALK, Some(&url), "One-hour lecture on scaling laws", hour);

    let calls: Vec<GeminiCall> = app.gemini.task_calls().into_iter().skip(before).collect();
    assert!(!calls.is_empty(), "the job never called Gemini");
    let input: u64 = calls.iter().map(|c| c.prompt_tokens).sum();
    let output: u64 = calls.iter().map(|c| c.output_tokens).sum();
    let (usd_in, usd_out, krw) = app.pricing();
    assert!(usd_in > 0.0 && usd_out > 0.0, "unpriced usage: {usd_in} / {usd_out}");
    assert!((500.0..3_000.0).contains(&krw), "implausible exchange rate {krw}");
    let expected = (input as f64 * usd_in + output as f64 * usd_out) / 1e6 * krw;

    let j = app.job(TALK);
    assert_eq!(j.body["input_amount"].as_u64(), Some(input), "input_amount vs Gemini usage: {j:?}");
    assert_eq!(j.body["output_amount"].as_u64(), Some(output), "output_amount vs Gemini usage (with thoughts): {j:?}");
    let cost = j.body["cost_krw"].as_f64().unwrap_or_else(|| panic!("cost_krw missing: {j:?}"));
    assert!((cost - expected).abs() <= 0.01, "cost_krw {cost} but usage x price x rate is {expected}");
    assert!(cost <= 1_000.0, "one hour cost {cost} KRW");
}

#[test]
fn test_cost_limit_stops_translation() {
    let mut app = App::new(Options { gemini: GeminiOptions { huge_usage: true, ..Default::default() }, ..Options::default() });
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(36_000, &regular_spans(45, 100, 800, 400)));
    app.launch();
    app.set_key(KEY);
    let before = app.gemini.generate_calls().len();
    let r = app.create_job(TALK, Some(&url), "A lecture with a runaway bill", 36_000);
    assert_eq!(r.status, 200, "POST /jobs: {r:?}");

    let seen = app.poll_job(TALK, 20, 180, |b| {
        assert_ne!(b["stage"], "ready", "translation finished past the cost limit: {b}");
        b["stage"] == "failed"
    });
    let last = seen.last().unwrap();
    assert_eq!(last["failure_reason"], "cost_limit", "{last}");
    sleep_ms(1_500);
    let made = app.gemini.generate_calls().len() - before;
    assert_eq!(made, 1, "Gemini was called {made} times; the first reply already broke the limit");
}

#[test]
fn test_terms_stay_consistent_across_batches() {
    let lines: Vec<String> = [
        "Here attention lets GPT-4o weigh every earlier token.",
        "GPT-4o stacks many attention layers.",
        "Attention scores in GPT-4o are normalised with a softmax.",
        "Without attention GPT-4o could not use long context.",
    ]
    .map(String::from)
    .to_vec();
    let mut app = App::new(Options { lines, ms_per_audio_second: 20, ..Options::default() });
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(150_000, &regular_spans(100, 100, 1_500, 500)));
    app.launch();
    app.set_key(KEY);
    let seen = app.run_job(TALK, Some(&url), "Attention in GPT-4o", 150_000);

    let batches: Vec<GeminiCall> = app.gemini.task_calls().into_iter().filter(|c| c.task.as_deref() == Some("translate")).collect();
    assert!(batches.len() >= 2, "100 cues went out in {} batch(es)", batches.len());
    let glossary = |c: &GeminiCall| {
        let mut g: Vec<(String, String)> = c.order.as_ref().unwrap()["glossary"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|e| (e["source"].as_str().unwrap_or("").to_lowercase(), e["target"].as_str().unwrap_or("").to_string()))
            .collect();
        g.sort();
        g
    };
    let first = glossary(&batches[0]);
    assert!(first.iter().any(|(s, _)| s == "attention"), "the glossary lacks `attention`: {first:?}");
    for b in &batches {
        assert_eq!(glossary(b), first, "batches carried different glossaries");
    }

    let video_key = seen[0]["video_key"].as_str().unwrap().to_string();
    let cues = cues(&app.subtitles(TALK, &video_key));
    let texts: Vec<String> = cues.iter().map(|c| c["text_ko"].as_str().unwrap_or("").to_string()).collect();
    let used: Vec<&str> = ["어텐션", "주의", "집중", "주목"].into_iter().filter(|w| texts.iter().any(|t| t.contains(w))).collect();
    assert_eq!(used.len(), 1, "`attention` is rendered as {used:?}");
    for t in &texts {
        assert!(t.contains("GPT-4o"), "the model name was translated: {t}");
    }
}
