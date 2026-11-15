use vto_contract_tests::{serde_json::json, *};

#[test]
fn site_playback_uses_p7_without_new_permissions_or_paid_calls_and_persists_after_restart() {
    let mut app = App::new(Options::default()); app.launch();
    let page = "https://lectures.example.org/playback";
    let created = app.app_call("POST", "/library/data", json!({"action":"create_playlist","name":"Playback"}));
    assert_eq!(created.status, 200);
    let library = app.app_call("GET", "/library/data", json!(null));
    let id = library.body["playlists"][0]["id"].as_str().unwrap();
    assert_eq!(app.app_call("POST", "/library/data", json!({"action":"add_entry","playlist_id":id,"page_url":page,"title":"Untranslated playback"})).status, 200);
    let read = app.call(page, "GET", "/playback", json!({"page_url":page}));
    assert_eq!(read.status, 200); assert!(read.body["position_ms"].is_null());
    let written = app.call(page, "PUT", "/playback", json!({"page_url":page,"position_ms":12345,"duration_ms":60000}));
    assert_eq!(written.status, 200, "{written:?}"); assert_eq!(written.body["position_ms"], 12345);
    assert_eq!(app.call(page, "PUT", "/playback", json!({"page_url":page,"position_ms":60001,"duration_ms":60000})).status, 400);
    assert_eq!(app.call(page, "GET", "/settings", json!(null)).status, 403);
    assert_eq!(app.call(page, "GET", "/library/data", json!(null)).status, 403);
    app.restart();
    let after = app.call(page, "GET", "/playback", json!({"page_url":page}));
    assert_eq!(after.status, 200, "{after:?}"); assert_eq!(after.body["position_ms"], 12345);
    let key = after.body["video_key"].as_str().unwrap();
    assert_eq!(app.app_call("POST", "/library/data", json!({"action":"delete_video","video_key":key,"title":"Untranslated playback","confirmed":true})).status, 200);
    assert_eq!(app.call(page, "GET", "/playback", json!({"page_url":page})).status, 410);
    assert_eq!(app.transcription_calls(), 0); assert!(app.gemini.calls().is_empty()); assert!(app.media.hits().is_empty());
}
