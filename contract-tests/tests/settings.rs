//! Settings: P3 GET /settings, P4 PUT and P5 DELETE /settings/api-key.

use vto_contract_tests::serde_json::json;
use vto_contract_tests::*;

const PAGE: &str = "https://lectures.example.org/talks/42";
const OTHER_KEY: &str = "AIzaSyZ9y8X7w6V5u4T3s2R1q0P9o8N7m6LQRST";

fn assert_masked(masked: &str) {
    assert!(masked.ends_with("WXYZ"), "key_masked {masked:?} does not end with the last four characters");
    let shown = &masked[..masked.len() - 4];
    assert!(!shown.chars().any(|c| c.is_ascii_alphanumeric()), "key_masked {masked:?} shows more than four characters");
}

#[test]
fn test_favorite_voice_is_not_offered() {
    let mut app = App::new(Options::default());
    app.launch();
    let fresh = app.settings();
    assert_eq!(fresh.body["favorite_voice_available"], false, "{fresh:?}");
    app.set_key(KEY);
    let keyed = app.settings();
    assert_eq!(keyed.body["favorite_voice_available"], false, "{keyed:?}");

    let commands = app.commands();
    assert!(commands.len() >= 8, "fewer commands than endpoints P1-P8: {commands:?}");
    let voice: Vec<_> = commands.iter().filter(|c| c.to_ascii_lowercase().contains("voice")).collect();
    assert!(voice.is_empty(), "voice commands are registered: {voice:?}");
}

#[test]
fn test_key_is_masked_and_deletable() {
    let mut app = App::new(Options::default());
    app.launch();
    let put = app.set_key(KEY);
    assert_eq!(put.body["key_set"], true, "{put:?}");
    assert_masked(put.body["key_masked"].as_str().unwrap_or(""));

    let s = app.settings();
    assert_eq!(s.body["key_set"], true, "{s:?}");
    assert_masked(s.body["key_masked"].as_str().unwrap_or(""));
    assert!(contains(&app.keystore_bytes(), KEY.as_bytes()), "the key is not in the secure store");
    assert!(!app.sqlite_files().is_empty(), "no SQLite file under the data folder");
    let leaks = app.data_files_containing(KEY.as_bytes());
    assert!(leaks.is_empty(), "the full key is stored in {leaks:?}");
    assert!(!contains(&app.logs(), KEY.as_bytes()), "the full key is in the logs");

    let d = app.app_call("DELETE", "/settings/api-key", json!(null));
    assert_eq!(d.status, 200, "{d:?}");
    assert_eq!(d.body["key_set"], false, "{d:?}");
    let s = app.settings();
    assert_eq!(s.body["key_set"], false, "{s:?}");
    assert!(s.body["key_masked"].is_null(), "{s:?}");
    assert!(!contains(&app.keystore_bytes(), KEY.as_bytes()), "the key stayed in the secure store");
    assert!(app.data_files_containing(KEY.as_bytes()).is_empty());
    assert!(!contains(&app.logs(), KEY.as_bytes()), "the full key is in the logs");
}

#[test]
fn test_site_page_cannot_call_key_commands() {
    let mut app = App::new(Options::default());
    app.launch();
    app.set_key(KEY);
    let opened = app.open(PAGE);
    assert_eq!(opened.status, 200, "POST /browser/open: {opened:?}");

    let put = app.call(PAGE, "PUT", "/settings/api-key", json!({ "api_key": OTHER_KEY }));
    assert!(put.status >= 400, "the site page replaced the key: {put:?}");
    let del = app.call(PAGE, "DELETE", "/settings/api-key", json!(null));
    assert!(del.status >= 400, "the site page deleted the key: {del:?}");
    let get = app.call(PAGE, "GET", "/settings", json!(null));
    assert!(get.status >= 400, "the site page read the settings: {get:?}");

    let s = app.settings();
    assert_eq!(s.body["key_set"], true, "{s:?}");
    assert_masked(s.body["key_masked"].as_str().unwrap_or(""));
    let store = app.keystore_bytes();
    assert!(contains(&store, KEY.as_bytes()) && !contains(&store, OTHER_KEY.as_bytes()), "the stored key changed");
    assert!(app.gemini.calls().iter().all(|c| c.api_key.as_deref() != Some(OTHER_KEY)), "the page's key reached Gemini");
}
