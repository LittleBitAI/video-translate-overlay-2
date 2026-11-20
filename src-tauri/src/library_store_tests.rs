use super::*;

#[test]
fn deletion_only_clears_the_browser_url_it_inspected() {
    let original = "https://lectures.example.org/a";
    for (label, navigation, expected) in [
        ("unchanged", original, None),
        ("unrelated", "https://lectures.example.org/b", Some("https://lectures.example.org/b")),
        ("new-position", "https://lectures.example.org/a#t=30", Some("https://lectures.example.org/a#t=30")),
        ("sign-in", "https://accounts.google.com/ServiceLogin", Some("https://accounts.google.com/ServiceLogin")),
    ] {
        let dir = std::env::temp_dir().join(format!("vto-delete-browser-{}-{}-{label}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.sqlite3");
        let s = Store::open(&path).unwrap();
        s.create_playlist("p", "Topic").unwrap();
        s.add_entry("p", "v", original, "other", "Lecture").unwrap();
        s.begin_delete("v", "token").unwrap();
        s.set_opened_url(original).unwrap();
        let inspected = s.opened_url().unwrap().unwrap();
        assert_eq!(inspected, original);
        let other = Store::open(&path).unwrap();
        other.set_opened_url(navigation).unwrap();
        s.finish_delete("v", Some(&inspected)).unwrap();
        assert_eq!(other.opened_url().unwrap().as_deref(), expected, "{label}");
        assert!(s.video("v").unwrap().is_none());
        assert_eq!(s.deleted("v").unwrap(), Some(("token".into(), true)));
        drop(other);
        drop(s);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn legacy_completed_rows_keep_timing_and_deletion_removes_all_owned_rows() {
    let dir = std::env::temp_dir().join(format!(
        "vto-library-upgrade-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.sqlite3");
    let db = Db::open(&path).unwrap();
    // The pre-library schema begins with video; no playlist or deletion tables exist.
    db.exec(&SCHEMA[SCHEMA.find("CREATE TABLE IF NOT EXISTS video").unwrap()..]).unwrap();
    db.exec(
        "INSERT INTO video VALUES ('v','https://example.org/a','other','Legacy lecture',10000,'en','glossary','old');
        INSERT INTO job (id,video_key,stage,translation_model,created_at,updated_at,cost_krw) VALUES ('j','v','ready','m','old','old',12.5);
        INSERT INTO cue VALUES ('v',0,1234,5678,'original','translated');
        INSERT INTO retranslation_cue VALUES ('j',0,'draft');",
    )
    .unwrap();
    drop(db);
    let s = Store::open(&path).unwrap();
    assert_eq!(s.library().unwrap()["videos"][0]["saved"], true);
    assert_eq!(s.cues("v").unwrap(), [Cue { seq: 0, start_ms: 1234, end_ms: 5678, source_text: "original".into(), text_ko: Some("translated".into()) }]);
    assert_eq!(s.latest_job("v").unwrap().unwrap().cost_krw, 12.5);
    s.create_playlist("p", "Topic").unwrap();
    s.add_entry("p", "v", "https://example.org/a", "other", "ignored").unwrap();
    s.begin_delete("v", "token").unwrap();
    s.finish_delete("v", None).unwrap();
    for table in ["video", "cue", "job", "retranslation_cue", "playlist_entry"] {
        assert!(s.db.query(&format!("SELECT * FROM {table}"), &[]).unwrap().is_empty(), "{table}");
    }
    assert!(s.playlist_exists("p").unwrap());
    assert_eq!(s.deleted("v").unwrap(), Some(("token".into(), true)));
    drop(s);
    std::fs::remove_dir_all(dir).unwrap();
}

/// The property: a summary row counts only for the video row it was made
/// for. A build without summaries deletes a video but leaves its summary
/// row, and the same key can come back as a new video row; that row's
/// summary and cost are not served, and a new call starts from nothing
/// instead of reviving the old body.
#[test]
fn a_summary_counts_only_for_the_video_row_it_was_made_for() {
    // What a build without summaries does to the video's own rows when it deletes it.
    const OLD_DELETE: &str = "DELETE FROM cue WHERE video_key='v'; DELETE FROM job WHERE video_key='v'; DELETE FROM video WHERE video_key='v';";
    const BACK: &str = "INSERT INTO video (video_key,page_url,site,title,duration_ms,created_at) VALUES ('v','https://example.org/a','other','Lecture',";
    let forms = [
        ("nothing happened", None, true),
        ("deleted and translated again by an older build", Some(format!("{OLD_DELETE}{BACK}1000,'later')")), false),
        ("deleted and added to a topic again by an older build", Some(format!("{OLD_DELETE}{BACK}0,'later')")), false),
        ("the video row put back with its own created_at", Some(format!("{OLD_DELETE}{BACK}1000,'first')")), true),
        ("a row from an earlier build of this branch", Some("UPDATE summary SET video_created_at=NULL".to_string()), false),
    ];
    for (name, step, kept) in forms {
        let dir = std::env::temp_dir().join(format!("vto-summary-row-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = Store::open(&dir.join("app.sqlite3")).unwrap();
        s.db.exec(&format!("{BACK}1000,'first')")).unwrap();
        s.open_summary_call("v", "c1").unwrap();
        assert!(s.record_summary("v", "c1", 10, 5, 1.5, Some("{\"gist\":\"old\"}"), None).unwrap());
        if let Some(step) = step { s.db.exec(&step).unwrap(); }
        let read = s.summary("v").unwrap();
        assert_eq!(read.as_ref().map(|r| (r.body.as_deref(), r.cost_krw)), kept.then_some((Some("{\"gist\":\"old\"}"), 1.5)), "{name}: read");
        // A new call on the video as it is now, whose reply breaks a rule: nothing stored, its usage billed.
        s.open_summary_call("v", "c2").unwrap();
        assert!(s.record_summary("v", "c2", 1, 1, 0.25, None, Some("broken")).unwrap(), "{name}");
        let after = s.summary("v").unwrap().unwrap();
        assert_eq!((after.body.is_some(), after.cost_krw), (kept, if kept { 1.75 } else { 0.25 }), "{name}: after a new call");
        drop(s);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
