//! The in-app browser: P1 POST /browser/open, P2 DELETE /browser/site-data,
//! YouTube sign-in, and opening a finished video from its notification.

use vto_contract_tests::serde_json::{json, Value};
use vto_contract_tests::*;

const YT: &str = "https://www.youtube.com/watch?v=contract0001";
const TALK: &str = "https://lectures.example.org/talks/42";
const SID: &str = "contract-sid-7f3a9c";

fn youtube_cookie(webview: &Value) -> Option<String> {
    webview["cookies"].as_array().cloned().unwrap_or_default().iter().find_map(|c| {
        let domain = c["domain"].as_str().unwrap_or("");
        (domain.ends_with("youtube.com") && c["name"] == "SID").then(|| c["value"].as_str().unwrap_or("").to_string())
    })
}

#[test]
fn test_ready_notification_names_video_and_opens_it() {
    let mut app = App::new(Options::default());
    let url = app.media.serve("/talks/42.wav", "audio/wav", speech_wav(12_000, &regular_spans(4, 500, 3_000, 2_000)));
    app.launch();
    app.set_key(KEY);
    let title = "Scaling Laws for Neural Language Models";
    let seen = app.run_job(TALK, Some(&url), title, 12_000);
    let job_id = seen[0]["job_id"].as_str().unwrap().to_string();

    let n = app.wait_notification("ready", &job_id);
    assert!(n["text"].as_str().unwrap_or("").contains(title), "the notification does not name the video: {n}");
    let target = n["url"].as_str().unwrap_or_else(|| panic!("the notification carries no url: {n}")).to_string();

    let opened = app.open(&target);
    assert_eq!(opened.status, 200, "POST /browser/open: {opened:?}");
    assert_eq!(opened.body["opened_url"], TALK, "{opened:?}");
    let w = app.webview();
    assert_eq!(w["url"], TALK, "the webview is elsewhere: {w}");
    assert_eq!(w["overlay"], true, "{w}");

    let j = app.job(TALK);
    assert_eq!(j.body["stage"], "ready", "{j:?}");
    assert_eq!(j.body["notified"], true, "{j:?}");
}

#[test]
fn test_open_url_shows_site_with_overlay() {
    let mut app = App::new(Options::default());
    app.launch();
    let r = app.open(TALK);
    assert_eq!(r.status, 200, "POST /browser/open: {r:?}");
    assert_eq!(r.body["opened_url"], TALK, "{r:?}");
    let w = app.webview();
    assert_eq!(w["url"], TALK, "{w}");
    assert_eq!(w["overlay"], true, "the overlay script is not attached: {w}");

    for bad in ["file:///C:/Windows/win.ini", "file:///etc/passwd", "javascript:alert(1)", "JavaScript:alert(document.cookie)"] {
        let r = app.open(bad);
        assert_eq!(r.status, 400, "{bad} was not refused: {r:?}");
    }
    let w = app.webview();
    assert_eq!(w["url"], TALK, "a refused url still navigated: {w}");
}

#[test]
fn test_first_youtube_open_shows_account_screen_once() {
    let mut app = App::new(Options::default());
    app.launch();
    let first = app.open(YT);
    assert_eq!(first.status, 200, "{first:?}");
    assert_eq!(first.body["account_gate"], true, "the first YouTube open skipped the account screen: {first:?}");
    let w = app.webview();
    assert!(w["url"].as_str().unwrap_or("").starts_with("https://accounts.google.com/"), "not Google's account screen: {w}");
    app.set_cookie("https://www.youtube.com", "SID", SID);

    let second = app.open(YT);
    assert_eq!(second.status, 200, "{second:?}");
    assert_eq!(second.body["account_gate"], false, "{second:?}");
    assert_eq!(second.body["opened_url"], YT, "{second:?}");
    assert_eq!(app.webview()["url"], YT);

    app.restart();
    let third = app.open(YT);
    assert_eq!(third.status, 200, "{third:?}");
    assert_eq!(third.body["account_gate"], false, "the account screen came back after a restart: {third:?}");
    let w = app.webview();
    assert_eq!(w["url"], YT, "{w}");
    assert_eq!(w["overlay"], true, "{w}");
    assert_eq!(youtube_cookie(&w).as_deref(), Some(SID), "the YouTube sign-in was lost: {w}");
    assert!(!app.data_files_containing(SID.as_bytes()).is_empty(), "the cookie is not kept in the app data folder");
}

#[test]
fn test_skipping_youtube_login_still_makes_subtitles() {
    let mut app = App::new(Options::default());
    app.media.serve("/yt/0001.wav", "audio/wav", speech_wav(12_000, &regular_spans(4, 500, 3_000, 2_000)));
    app.youtube(YT, "/yt/0001.wav");
    app.launch();
    app.set_key(KEY);

    let gate = app.open(YT);
    assert_eq!(gate.body["account_gate"], true, "{gate:?}");
    let skipped = app.open(YT); // the user skipped Google's screen
    assert_eq!(skipped.status, 200, "{skipped:?}");
    assert_eq!(skipped.body["account_gate"], false, "{skipped:?}");
    let w = app.webview();
    assert_eq!(w["url"], YT, "{w}");
    assert_eq!(youtube_cookie(&w), None, "there should be no sign-in: {w}");

    let seen = app.run_job(YT, None, "Attention Is All You Need, explained", 12_000);
    let video_key = seen[0]["video_key"].as_str().unwrap().to_string();
    assert!(!cues(&app.subtitles(YT, &video_key)).is_empty(), "no subtitles without a YouTube sign-in");
}

#[test]
fn test_clearing_site_data_ends_youtube_login() {
    let mut app = App::new(Options::default());
    app.launch();
    assert_eq!(app.open(YT).body["account_gate"], true);
    app.set_cookie("https://www.youtube.com", "SID", SID);
    assert_eq!(app.open(YT).body["account_gate"], false);
    assert_eq!(youtube_cookie(&app.webview()).as_deref(), Some(SID));

    let cleared = app.app_call("DELETE", "/browser/site-data", json!(null));
    assert_eq!(cleared.status, 200, "{cleared:?}");
    assert_eq!(cleared.body["cleared"], true, "{cleared:?}");
    assert_eq!(youtube_cookie(&app.webview()), None, "the YouTube cookie survived");
    assert!(app.data_files_containing(SID.as_bytes()).is_empty(), "the cookie is still on disk");

    let again = app.open(YT);
    assert_eq!(again.body["account_gate"], true, "the account screen did not come back: {again:?}");
    assert!(app.webview()["url"].as_str().unwrap_or("").starts_with("https://accounts.google.com/"));

    let s = app.settings();
    let keys: Vec<&String> = s.body.as_object().expect("settings object").keys().collect();
    for k in &keys {
        let k = k.to_ascii_lowercase();
        assert!(!k.contains("password") && !k.contains("passwd"), "settings carry a password field: {keys:?}");
    }
    assert!(!app.sqlite_files().is_empty(), "no SQLite file under the data folder");
    for f in app.sqlite_files() {
        let bytes = std::fs::read(&f).unwrap();
        assert!(!contains_ci(&bytes, "password") && !contains_ci(&bytes, "passwd"), "{f:?} has a password column");
    }
}
