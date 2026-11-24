//! P19, the 요약 button: one schema-constrained Gemini call over a ready
//! video's title, source transcript and glossary; checked, stored, metered.

use serde_json::{json, Value};
use vto_contract_tests::*;

const TALK: &str = "https://lectures.example.org/talks/42";
const TITLE: &str = "Transformers in twelve seconds";

/// A ready video on `app`, and its key.
fn ready(app: &mut App) -> String {
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(12_000, &regular_spans(4, 500, 3_000, 2_000)));
    app.launch();
    app.set_key(KEY);
    let seen = app.run_job(TALK, Some(&url), TITLE, 12_000);
    seen[0]["video_key"].as_str().unwrap().to_string()
}

fn summarize(app: &mut App, key: &str, again: bool) -> Resp {
    app.app_call("POST", &format!("/videos/{key}/summary"), if again { json!({"again": true}) } else { json!({}) })
}

fn summary_calls(app: &App) -> Vec<GeminiCall> {
    app.gemini.task_calls().into_iter().filter(|c| c.task.as_deref() == Some("summary")).collect()
}

fn krw(app: &mut App, calls: &[GeminiCall]) -> f64 {
    let (usd_in, usd_out, rate) = app.pricing();
    calls.iter().map(|c| (c.prompt_tokens as f64 * usd_in + c.output_tokens as f64 * usd_out) / 1e6 * rate).sum()
}

#[test]
fn test_summary_request_carries_title_source_transcript_terms_and_schema() {
    let mut app = App::new(Options::default());
    let key = ready(&mut app);
    let all = app.gemini.task_calls();
    let translated = krw(&mut app, &all);
    let made = summarize(&mut app, &key, false);
    assert_eq!(made.status, 200, "{made:?}");
    assert_eq!(made.body["reused"], false);

    let calls = summary_calls(&app);
    assert_eq!(calls.len(), 1, "one summary call");
    let call = &calls[0];
    let order = call.order.as_ref().unwrap();
    assert_eq!(call.model.as_deref(), Some(MODEL));
    // The summary's own prompt, not the translator's.
    assert!(call.system.contains("You summarize one technical talk"), "{}", call.system);
    assert!(!call.system.contains("You translate subtitles"), "{}", call.system);

    // Title, every cue's source text in order (what the glossary request was given), and the job's terms.
    let glossary_call = app.gemini.task_calls().into_iter().find(|c| c.task.as_deref() == Some("glossary")).unwrap();
    let translate_call = app.gemini.task_calls().into_iter().find(|c| c.task.as_deref() == Some("translate")).unwrap();
    let mut fields: Vec<&str> = order.as_object().unwrap().keys().map(String::as_str).collect();
    fields.sort();
    assert_eq!(fields, ["task", "terms", "title", "transcript"], "{order}");
    assert_eq!(order["title"], TITLE);
    assert_eq!(order["transcript"], glossary_call.order.as_ref().unwrap()["transcript"]);
    assert!(order["transcript"].as_array().unwrap().iter().all(Value::is_string), "a transcript entry is not plain text: {order}");
    assert_eq!(order["terms"], translate_call.order.as_ref().unwrap()["glossary"]);
    assert!(!order["terms"].as_array().unwrap().is_empty(), "the fake glossary found no terms");

    // No Korean cue text and no timestamps anywhere in what was sent.
    let subtitles = cues(&app.subtitles(TALK, &key));
    assert!(!subtitles.is_empty());
    for c in &subtitles {
        let ko = c["text_ko"].as_str().unwrap();
        assert!(!call.body.contains(ko), "Korean cue text was sent: {ko}");
    }
    assert!(!call.body.contains("한국어 번역"), "translated text was sent");
    for t in ["start_ms", "end_ms", "seq"] {
        assert!(!call.body.contains(t), "a timing field was sent: {t}");
    }

    // A response JSON schema fixes the fields, the level's values and each field's language.
    let schema = &call.generation_config["responseJsonSchema"];
    assert_eq!(call.generation_config["responseMimeType"], "application/json");
    let mut required: Vec<&str> = schema["required"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    required.sort();
    assert_eq!(required, ["gist", "key_points", "level", "next_topics", "prerequisites", "topics"]);
    assert_eq!(schema["properties"]["level"]["enum"], json!(["beginner", "intermediate", "advanced"]));
    for field in ["gist", "key_points", "prerequisites"] {
        assert!(schema["properties"][field]["description"].as_str().unwrap().contains("in Korean"), "{field}");
    }
    for field in ["topics", "next_topics"] {
        assert!(schema["properties"][field]["description"].as_str().unwrap().contains("in the talk's own language"), "{field}");
    }

    // Stored, rendered, and its cost added to the video's shown cost.
    let summary = &made.body["summary"];
    assert!(has_hangul(summary["gist"].as_str().unwrap()));
    assert_eq!(summary["topics"], json!(["attention", "transformer", "gradient descent"]));
    assert!(made.body["markdown"].as_str().unwrap().starts_with("## 한 줄 요약\n\n"));
    let summarized = krw(&mut app, &calls);
    let job = app.job(TALK).body;
    assert!((job["summary_cost_krw"].as_f64().unwrap() - summarized).abs() <= 1e-6, "{job}");
    assert!((job["cost_krw"].as_f64().unwrap() - (translated + summarized)).abs() <= 0.01, "{job}");

    // 요약 again shows the stored one without a call; re-summarizing is its own request.
    let shown = summarize(&mut app, &key, false);
    assert_eq!((shown.status, &shown.body["summary"], &shown.body["markdown"], &shown.body["reused"]), (200, summary, &made.body["markdown"], &json!(true)));
    let read = app.app_call("GET", &format!("/videos/{key}/summary"), Value::Null);
    assert_eq!((read.status, &read.body["markdown"]), (200, &made.body["markdown"]));
    assert_eq!(summary_calls(&app).len(), 1, "a stored summary was asked for again");
    let again = summarize(&mut app, &key, true);
    assert_eq!((again.status, &again.body["reused"]), (200, &json!(false)), "{again:?}");
    let calls = summary_calls(&app);
    assert_eq!(calls.len(), 2);
    let job = app.job(TALK).body;
    assert!((job["cost_krw"].as_f64().unwrap() - (translated + krw(&mut app, &calls))).abs() <= 0.01, "{job}");

    // A restart keeps it.
    app.restart();
    let read = app.app_call("GET", &format!("/videos/{key}/summary"), Value::Null);
    assert_eq!((read.status, &read.body["summary"]), (200, &again.body["summary"]));
}

#[test]
fn test_a_reply_that_breaks_a_value_rule_is_not_stored_but_is_metered() {
    for fault in ["empty_topics", "bad_level", "empty_gist"] {
        let mut app = App::new(Options { gemini: GeminiOptions { summary_fault: Some(fault), ..Default::default() }, ..Options::default() });
        let key = ready(&mut app);
        let all = app.gemini.task_calls();
    let translated = krw(&mut app, &all);
        let refused = summarize(&mut app, &key, false);
        assert_eq!((refused.status, refused.body["error"].as_str()), (502, Some("summary_invalid")), "{fault}: {refused:?}");
        let read = app.app_call("GET", &format!("/videos/{key}/summary"), Value::Null);
        // Nothing stored: no summary and no Markdown, only why the call stored nothing.
        assert_eq!((read.status, &read.body["summary"], &read.body["markdown"]), (200, &Value::Null, &Value::Null), "{fault}: a broken summary was stored: {read:?}");
        assert!(read.body["failure"].as_str().is_some_and(|f| f.contains("저장하지 않았습니다")), "{fault}: {read:?}");
        let calls = summary_calls(&app);
        assert_eq!(calls.len(), 1, "{fault}");
        let job = app.job(TALK).body;
        let metered = krw(&mut app, &calls);
        assert!(metered > 0.0);
        assert!((job["cost_krw"].as_f64().unwrap() - (translated + metered)).abs() <= 0.01, "{fault}: {job}");
        // Retrying calls again.
        assert_eq!(summarize(&mut app, &key, false).status, 502);
        assert_eq!(summary_calls(&app).len(), 2, "{fault}");
    }
}

#[test]
fn test_only_a_ready_video_on_the_app_screen_can_be_summarized() {
    let mut app = App::new(Options { gemini: GeminiOptions { delay_ms: 400, ..Default::default() }, ..Options::default() });
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(12_000, &regular_spans(4, 500, 3_000, 2_000)));
    app.launch();
    app.set_key(KEY);

    // An untranslated playlist link.
    const LINK: &str = "https://lectures.example.org/talks/untranslated";
    assert_eq!(app.app_call("POST", "/library/data", json!({"action": "create_playlist", "name": "Later"})).status, 200);
    let library = app.app_call("GET", "/library/data", Value::Null).body;
    let playlist = library["playlists"][0]["id"].as_str().unwrap().to_string();
    let added = app.app_call("POST", "/library/data", json!({"action": "add_entry", "playlist_id": playlist, "page_url": LINK, "title": "Untranslated"}));
    assert_eq!(added.status, 200, "{added:?}");
    let library = app.app_call("GET", "/library/data", Value::Null).body;
    let link_key = library["videos"].as_array().unwrap().iter().find(|v| v["page_url"] == LINK).unwrap()["video_key"].as_str().unwrap().to_string();
    let r = summarize(&mut app, &link_key, false);
    assert_eq!((r.status, r.body["error"].as_str()), (409, Some("not_ready")), "{r:?}");

    // A video still processing.
    let created = app.create_job(TALK, Some(&url), TITLE, 12_000);
    let key = created.body["video_key"].as_str().unwrap().to_string();
    app.poll_job(TALK, 20, 120, |b| b["stage"] == "translating");
    let r = summarize(&mut app, &key, false);
    assert_eq!((r.status, r.body["error"].as_str()), (409, Some("not_ready")), "{r:?}");

    // Ready, but asked from the site page: the overlay may not spend the key on it.
    app.wait_ready(TALK);
    let r = app.call(TALK, "POST", &format!("/videos/{key}/summary"), json!({}));
    assert!(r.status >= 400, "the site page summarized: {r:?}");
    assert!(summary_calls(&app).is_empty(), "a refused request reached Gemini");
    assert_eq!(app.app_call("GET", &format!("/videos/{key}/summary"), Value::Null).status, 404);
    assert_eq!(summarize(&mut app, &key, false).status, 200);
}
