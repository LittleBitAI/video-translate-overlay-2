use vto_contract_tests::serde_json::{json, Value};
use vto_contract_tests::*;

const A: &str = "https://lectures.example.org/a";
const B: &str = "https://lectures.example.org/b";

fn library(app: &mut App) -> Value {
    let r = app.app_call("GET", "/library/data", Value::Null);
    assert_eq!(r.status, 200, "{r:?}");
    r.body
}

fn change(app: &mut App, action: Value) {
    let r = app.app_call("POST", "/library/data", action);
    assert_eq!(r.status, 200, "{r:?}");
}

fn playlist(app: &mut App, name: &str) -> String {
    change(app, json!({"action":"create_playlist","name":name}));
    library(app)["playlists"].as_array().unwrap().iter().find(|p| p["name"] == name).unwrap()["id"].as_str().unwrap().to_string()
}

fn translated(app: &mut App, page: &str, title: &str) -> Resp {
    let url = app.media.serve(&format!("/{title}.wav"), "audio/wav", speech_wav(5000, &[(500, 3500)]));
    let r = app.create_job(page, Some(&url), title, 5000);
    assert_eq!(r.status, 200, "{r:?}");
    app.wait_ready(page);
    r
}

#[test]
fn saved_library_reopens_after_restart_without_transcription_translation_or_cost() {
    let mut app = App::new(Options::default());
    app.launch();
    app.set_key(KEY);
    let a = translated(&mut app, A, "Attention");
    let cues = app.subtitles(A, &a.str("video_key")).body;
    let cost = app.job(A).body["cost_krw"].clone();
    let calls = app.gemini.calls().len();
    let hits = app.media.hits().len();
    app.restart();
    assert_eq!(app.transcription_calls(), 0);
    let list = library(&mut app);
    assert_eq!(list["videos"][0]["title"], "Attention");
    assert_eq!(list["videos"][0]["saved"], true);
    assert_eq!(app.open(A).status, 200);
    assert_eq!(app.webview()["url"], A);
    assert_eq!(app.subtitles(A, &a.str("video_key")).body, cues);
    assert_eq!(app.create_job(A, None, "ignored", 5000).body["reused"], true);
    assert_eq!(app.job(A).body["cost_krw"], cost);
    assert_eq!(app.transcription_calls(), 0);
    assert_eq!(app.gemini.calls().len(), calls);
    assert_eq!(app.media.hits().len(), hits);
}

#[test]
fn playlist_crud_order_and_untranslated_links_persist_without_paid_calls() {
    let mut app = App::new(Options::default());
    app.launch();
    let p = playlist(&mut app, "Deep learning");
    let q = playlist(&mut app, "Systems");
    change(&mut app, json!({"action":"rename_playlist","playlist_id":p,"name":"Attention"}));
    change(&mut app, json!({"action":"move_playlist","playlist_id":q,"delta":-1}));
    for page in [A, B] {
        change(&mut app, json!({"action":"add_entry","playlist_id":p,"page_url":page}));
    }
    change(&mut app, json!({"action":"add_entry","playlist_id":q,"page_url":A}));
    let key = library(&mut app)["playlists"][1]["entries"][1].clone();
    change(&mut app, json!({"action":"move_entry","playlist_id":p,"video_key":key,"delta":-1}));
    let before = library(&mut app);
    app.restart();
    assert_eq!(library(&mut app), before);
    assert_eq!(before["playlists"][0]["id"], q);
    assert_eq!(before["playlists"][1]["entries"][0], key);
    assert_eq!(app.open(A).status, 200);
    assert_eq!(app.job(A).status, 404);
    change(&mut app, json!({"action":"remove_entry","playlist_id":p,"video_key":key}));
    change(&mut app, json!({"action":"delete_playlist","playlist_id":q}));
    assert_eq!(library(&mut app)["videos"].as_array().unwrap().len(), 2);
    assert!(app.gemini.calls().is_empty());
    assert_eq!(app.transcription_calls(), 0);
    let refused = app.create_job(A, None, "A", 5000);
    assert_eq!(refused.status, 428);
    assert!(app.call(A, "GET", "/library/data", Value::Null).status >= 400);
    assert!(app.call(A, "POST", "/library/data", json!({"action":"create_playlist","name":"outside"})).status >= 400);
    for bad in ["file:///outside", "javascript:alert(1)", "https://www.youtube.com/"] {
        assert_eq!(app.app_call("POST", "/library/data", json!({"action":"add_entry","playlist_id":p,"page_url":bad})).status, 400);
    }
}

#[test]
fn deletion_confirmation_retry_and_multiple_memberships_preserve_unrelated_data() {
    let mut app = App::new(Options::default());
    app.launch();
    app.set_key(KEY);
    let a = translated(&mut app, A, "Attention");
    let b = translated(&mut app, B, "Systems");
    let key = a.str("video_key");
    let job = a.str("job_id");
    let other = app.subtitles(B, &b.str("video_key")).body;
    let p = playlist(&mut app, "Topic1");
    let q = playlist(&mut app, "Topic2");
    for id in [&p, &q] {
        for page in [A, B] {
            change(&mut app, json!({"action":"add_entry","playlist_id":id,"page_url":page}));
        }
    }
    let before = library(&mut app);
    assert_eq!(app.app_call("POST", "/library/data", json!({"action":"delete_video","video_key":key,"title":"Attention"})).status, 400);
    assert_eq!(library(&mut app), before, "cancelled deletion changed data");
    assert_eq!(app.subtitles(A, &key).status, 200);
    let model = app.data_dir().join("shared-model.bin");
    std::fs::write(&model, b"shared model").unwrap();
    let audio = app.data_dir().join("audio");
    let held = audio.join(format!("{job}.blocked"));
    std::fs::create_dir(&held).unwrap();
    std::fs::write(audio.join(format!("{job}.wav")), b"audio").unwrap();
    std::fs::write(audio.join(format!("{job}.wav.sections")), b"index").unwrap();
    let deletion = json!({"action":"delete_video","video_key":key,"title":"Attention","confirmed":true});
    assert_eq!(app.app_call("POST", "/library/data", deletion.clone()).body["error"], "cleanup_failed");
    assert_eq!(app.subtitles(A, &key).status, 410);
    assert_eq!(app.app_call("POST", "/jobs", json!({"page_url":A,"retranslate":true})).status, 410);
    app.restart();
    assert_eq!(app.job(A).status, 410);
    assert!(library(&mut app)["videos"].as_array().unwrap().iter().any(|v| v["video_key"] == key && v["deleting"] == true));
    std::fs::remove_dir(&held).unwrap();
    change(&mut app, deletion);
    assert!(!audio.join(format!("{job}.wav")).exists());
    assert!(!audio.join(format!("{job}.wav.sections")).exists());
    let after = library(&mut app);
    assert_eq!(after["videos"].as_array().unwrap().len(), 1);
    for list in after["playlists"].as_array().unwrap() {
        assert_eq!(list["entries"], json!([b.str("video_key")]));
    }
    assert_eq!(std::fs::read(model).unwrap(), b"shared model");
    assert_eq!(app.settings().body["key_set"], true);
    assert_eq!(app.subtitles(B, &b.str("video_key")).body, other);
    assert_eq!(app.create_job(A, None, "Attention", 5000).status, 410, "a stale create must not recreate a deleted job");
    assert_eq!(app.open(A).status, 200, "old links may open the source, without saved data");
    assert_eq!(app.subtitles(A, &key).status, 410);
    app.restart();
    assert_eq!(library(&mut app), after);
    assert_eq!(app.job(A).status, 410);
    assert_eq!(app.transcription_calls(), 0);
    let token = app.job(A).body["deletion_token"].clone();
    app.app_call("DELETE", "/settings/api-key", Value::Null);
    let consent = json!({"page_url":A,"duration_ms":5000,"deletion_token":token});
    assert_eq!(app.app_call("POST", "/jobs", consent.clone()).status, 428, "fresh consent still requires a key");
    assert_eq!(app.job(A).body["deletion_token"], token, "failed admission must preserve deletion");
    app.set_key(KEY);
    let url = app.media.url("/Attention.wav");
    let r = app.app_call("POST", "/jobs", json!({"page_url":A,"duration_ms":5000,"deletion_token":token,"media_url":url,"title":"Attention"}));
    assert_eq!(r.status, 200, "fresh explicit consent can translate again: {r:?}");
    app.wait_ready(A);
    change(&mut app, json!({"action":"delete_video","video_key":key,"title":"Attention","confirmed":true}));
    assert_eq!(app.app_call("POST", "/jobs", consent).status, 410, "a previous deletion token cannot admit a later job");
}

#[test]
fn deletion_clears_only_matching_open_video_urls() {
    for (opened, expected) in [
        (None, None),
        (Some(A), None),
        (Some("https://lectures.example.org/a?utm_source=share#t=30"), None),
        (Some(B), Some(B)),
    ] {
        let mut app = App::new(Options::default());
        app.launch();
        let p = playlist(&mut app, "Topic");
        change(&mut app, json!({"action":"add_entry","playlist_id":p,"page_url":A,"title":"Lecture"}));
        let key = library(&mut app)["videos"][0]["video_key"].clone();
        if let Some(url) = opened { assert_eq!(app.open(url).status, 200); }
        change(&mut app, json!({"action":"delete_video","video_key":key,"title":"Lecture","confirmed":true}));
        let browser = app.app_call("GET", "/browser", Value::Null);
        assert_eq!(browser.status, 200);
        assert_eq!(browser.body["url"], json!(expected));
        assert!(library(&mut app)["videos"].as_array().unwrap().is_empty());
    }
}

#[test]
fn deletion_rejects_active_processing() {
    let mut app = App::new(Options::default());
    let url = app.media.serve("/held.wav", "audio/wav", speech_wav(5000, &[(500, 3500)]));
    app.media.hold("/held.wav");
    app.launch();
    app.set_key(KEY);
    let a = app.create_job(A, Some(&url), "Active", 5000);
    assert_eq!(a.status, 200);
    let r = app.app_call("POST", "/library/data", json!({"action":"delete_video","video_key":a.str("video_key"),"title":"Active","confirmed":true}));
    assert_eq!(r.status, 409);
    assert_eq!(r.body["error"], "video_active");
    app.media.release("/held.wav");
    app.wait_ready(A);
}
