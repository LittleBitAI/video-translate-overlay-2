use super::*;

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
    s.finish_delete("v", false).unwrap();
    for table in ["video", "cue", "job", "retranslation_cue", "playlist_entry"] {
        assert!(s.db.query(&format!("SELECT * FROM {table}"), &[]).unwrap().is_empty(), "{table}");
    }
    assert!(s.playlist_exists("p").unwrap());
    assert_eq!(s.deleted("v").unwrap(), Some(("token".into(), true)));
    drop(s);
    std::fs::remove_dir_all(dir).unwrap();
}
