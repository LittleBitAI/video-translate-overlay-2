use super::*;
use crate::{google_auth::VerifiedIdentity, p};
use std::path::PathBuf;

struct Profile(PathBuf);
impl Profile {
    fn new() -> Self {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).unwrap();
        let path = std::env::temp_dir().join(format!("vto-sync-library-{}", digest(&bytes)));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn journal(&self) -> Journal {
        Journal::open(&self.0, &VerifiedIdentity::fixture("same-google-account")).unwrap()
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn translation() -> Change {
    let page_url = "https://lectures.example.org/projected";
    Change::Translation {
        translation: Translation {
            video: Video {
                key: crate::api::video_key(&url::Url::parse(page_url).unwrap()),
                page_url: page_url.into(),
                title: "Original lecture".into(),
            },
            duration_ms: 5000,
            source_language: Some("en".into()),
            terms: vec![Term {
                source: "attention".into(),
                target: "saved term".into(),
            }],
            model: crate::gemini::MODEL.into(),
            cost_krw: 12.5,
            cues: vec![Cue {
                start_ms: 1234,
                end_ms: 4321,
                source_text: "Original source".into(),
                text_ko: "Saved translation".into(),
            }],
        },
    }
}
fn video_key() -> String {
    translation().video_key().unwrap().into()
}
fn send(from: &Journal, to: &Journal) {
    let page = from.page(to.cursor(from.replica()).unwrap()).unwrap();
    to.receive(from.replica(), &page).unwrap();
}

#[test]
fn received_ready_library_is_usable_without_processing_and_survives_restart() {
    let a = Profile::new();
    let b = Profile::new();
    let first = a.journal();
    let second = b.journal();
    for change in [
        translation(),
        Change::PlaylistName {
            id: "topic".into(),
            name: "Saved topic".into(),
        },
        Change::EntryPresence {
            id: "topic".into(),
            key: video_key(),
            present: true,
        },
        Change::Playback {
            key: video_key(),
            position_ms: 2345,
        },
    ] {
        first.write(change, Deletions::new()).unwrap();
    }
    send(&first, &second);
    let checkpoint = second.cursor(first.replica()).unwrap();
    drop(second);
    let second = b.journal();
    let store = second.library().unwrap();
    assert_eq!(second.cursor(first.replica()).unwrap(), checkpoint);
    assert_eq!(second.pending_projection().unwrap(), 0);
    assert_eq!(
        store.library().unwrap()["playlists"][0]["entries"][0],
        video_key()
    );
    let job = store.ready_job(&video_key()).unwrap().unwrap();
    assert_eq!(job.cost_krw, 12.5);
    assert_eq!(job.stage, "ready");
    assert!(!job.notified);
    assert!(job.audio_path.is_none());
    assert!(store.active_job().unwrap().is_none());
    let cues = store.cues(&video_key()).unwrap();
    assert_eq!((cues[0].start_ms, cues[0].end_ms), (1234, 4321));
    assert_eq!(cues[0].source_text, "Original source");
    assert_eq!(cues[0].text_ko.as_deref(), Some("Saved translation"));
    assert_eq!(
        Db::open(store.path())
            .unwrap()
            .query_one(
                "SELECT position_ms FROM playback WHERE video_key=?",
                p![video_key()]
            )
            .unwrap()
            .unwrap()[0]
            .int(),
        2345
    );
    send(&first, &second);
    assert_eq!(second.pending_projection().unwrap(), 0);
}

#[test]
fn failed_projection_rolls_back_received_records_and_cursor_together() {
    let a = Profile::new();
    let b = Profile::new();
    let first = a.journal();
    let second = b.journal();
    first.write(translation(), Deletions::new()).unwrap();
    let db = Db::open(&second.path).unwrap();
    // A storage failure after cue insertion must undo the projection and its transfer receipt.
    db.execute("CREATE TRIGGER refuse_ready BEFORE INSERT ON job BEGIN SELECT RAISE(ABORT,'fixture write failure'); END",&[]).unwrap();
    let page = first.page(0).unwrap();
    assert!(second.receive(first.replica(), &page).is_err());
    assert_eq!(second.cursor(first.replica()).unwrap(), 0);
    assert!(second.records().unwrap().is_empty());
    assert!(second
        .library()
        .unwrap()
        .video(&video_key())
        .unwrap()
        .is_none());
    db.execute("DROP TRIGGER refuse_ready", &[]).unwrap();
    send(&first, &second);
    assert_eq!(
        second
            .library()
            .unwrap()
            .ready_job(&video_key())
            .unwrap()
            .unwrap()
            .cost_krw,
        12.5
    );
}

#[test]
fn active_jobs_defer_projection_and_confirmed_deletion_stays_hidden_until_cleanup() {
    let a = Profile::new();
    let b = Profile::new();
    let first = a.journal();
    let second = b.journal();
    first.write(translation(), Deletions::new()).unwrap();
    send(&first, &second);
    let store = second.library().unwrap();
    let video = store.video(&video_key()).unwrap().unwrap();
    store
        .create_job(
            "local-active",
            &video_key(),
            &video.page_url,
            "other",
            &video.title,
            5000,
            None,
            crate::gemini::MODEL,
        )
        .unwrap();
    first
        .write(Change::DeleteVideo { key: video_key() }, Deletions::new())
        .unwrap();
    send(&first, &second);
    assert_eq!(second.pending_projection().unwrap(), 1);
    assert!(store.deleted(&video_key()).unwrap().is_none());
    assert!(store.active_job().unwrap().is_some());
    store.fail("local-active", "fixture_finished").unwrap();
    assert_eq!(second.project_pending().unwrap(), 1);
    assert_eq!(store.deleted(&video_key()).unwrap().unwrap().1, false);
    assert_eq!(store.library().unwrap()["videos"][0]["saved"], false);
    let token = store.deleted(&video_key()).unwrap().unwrap().0;
    second.project_pending().unwrap();
    assert_eq!(store.deleted(&video_key()).unwrap().unwrap().0, token);
    store.finish_delete(&video_key(), None).unwrap();
    assert!(store.video(&video_key()).unwrap().is_none());
    assert!(store.cues(&video_key()).unwrap().is_empty());
    let original = first.records().unwrap();
    assert!(original
        .iter()
        .any(|r| matches!(r.change, Change::DeleteVideo { .. })));
}

#[test]
fn legacy_import_is_a_copy_is_idempotent_and_cannot_assign_another_accounts_store() {
    let source = Profile::new();
    let target = Profile::new();
    let store = Store::open(&source.0.join("app.sqlite3")).unwrap();
    let Change::Translation { translation: t } = translation() else {
        unreachable!()
    };
    store
        .create_job(
            "legacy-ready",
            &t.video.key,
            &t.video.page_url,
            "other",
            &t.video.title,
            t.duration_ms,
            None,
            &t.model,
        )
        .unwrap();
    store.set_language(&t.video.key, "en").unwrap();
    store
        .set_terms(&t.video.key, &serde_json::to_string(&t.terms).unwrap())
        .unwrap();
    let db = Db::open(store.path()).unwrap();
    db.execute(
        "INSERT INTO cue VALUES (?,?,?,?,?,?)",
        p![
            &t.video.key,
            0i64,
            1234i64,
            4321i64,
            "Original source",
            "Saved translation"
        ],
    )
    .unwrap();
    db.execute(
        "UPDATE job SET stage='ready',cost_krw=12.5 WHERE id='legacy-ready'",
        &[],
    )
    .unwrap();
    store.create_playlist("topic", "Device topic").unwrap();
    store
        .add_entry(
            "topic",
            &t.video.key,
            &t.video.page_url,
            "other",
            &t.video.title,
        )
        .unwrap();
    let original = store.library().unwrap();
    let journal = target.journal();
    assert!(journal.import_legacy(&store).unwrap() > 0);
    assert_eq!(store.library().unwrap(), original);
    assert_eq!(
        store.ready_job(&t.video.key).unwrap().unwrap().id,
        "legacy-ready"
    );
    assert_eq!(journal.import_legacy(&store).unwrap(), 0);
    let copied = journal.library().unwrap();
    assert_eq!(
        copied.ready_job(&t.video.key).unwrap().unwrap().cost_krw,
        12.5
    );
    assert_eq!(
        copied.cues(&t.video.key).unwrap(),
        store.cues(&t.video.key).unwrap()
    );
    let other_profile = Profile::new();
    let other = other_profile.journal();
    assert!(other.import_legacy(&copied).is_err());
    journal
        .write(
            Change::DeleteVideo {
                key: t.video.key.clone(),
            },
            Deletions::new(),
        )
        .unwrap();
    assert_eq!(journal.import_legacy(&store).unwrap(), 0);
    assert!(journal
        .library()
        .unwrap()
        .deleted(&t.video.key)
        .unwrap()
        .is_some());
}

#[test]
fn partial_dependencies_survive_restart_and_blocked_rows_do_not_starve_later_work() {
    let source = Profile::new();
    let target = Profile::new();
    let first = source.journal();
    let second = target.journal();
    first
        .write(
            Change::EntryPresence {
                id: "topic".into(),
                key: video_key(),
                present: true,
            },
            Deletions::new(),
        )
        .unwrap();
    send(&first, &second);
    assert_eq!(second.pending_projection().unwrap(), 1);
    drop(second);
    let second = target.journal();
    second
        .transaction(|db| {
            for i in 0..130 {
                db.execute(
                    "INSERT INTO sync_dirty VALUES ('playlist',?)",
                    p![format!("missing-{i}")],
                )?;
            }
            Ok(())
        })
        .unwrap();
    first.write(translation(), Deletions::new()).unwrap();
    first
        .write(
            Change::PlaylistName {
                id: "topic".into(),
                name: "Arrived later".into(),
            },
            Deletions::new(),
        )
        .unwrap();
    send(&first, &second);
    for _ in 0..4 {
        second.project_pending().unwrap();
    }
    assert_eq!(
        second.library().unwrap().library().unwrap()["playlists"][0]["entries"][0],
        video_key()
    );
    assert_eq!(second.pending_projection().unwrap(), 130);
}

#[test]
fn concurrent_completed_versions_are_retained_and_fresh_restoration_waits_for_cleanup() {
    let source = Profile::new();
    let target = Profile::new();
    let first = source.journal();
    let second = target.journal();
    first.write(translation(), Deletions::new()).unwrap();
    let Change::Translation {
        translation: mut alternate,
    } = translation()
    else {
        unreachable!()
    };
    alternate.cues[0].text_ko = "Independent completed response".into();
    second
        .write(
            Change::Translation {
                translation: alternate,
            },
            Deletions::new(),
        )
        .unwrap();
    send(&first, &second);
    send(&second, &first);
    assert_eq!(
        first.library().unwrap().cues(&video_key()).unwrap(),
        second.library().unwrap().cues(&video_key()).unwrap()
    );
    assert_eq!(
        second
            .records()
            .unwrap()
            .iter()
            .filter(|r| matches!(r.change, Change::Translation { .. }))
            .count(),
        2
    );
    first
        .write(Change::DeleteVideo { key: video_key() }, Deletions::new())
        .unwrap();
    send(&first, &second);
    let context = first.deletions(&video_key()).unwrap();
    first.write(translation(), context).unwrap();
    send(&first, &second);
    let store = second.library().unwrap();
    assert!(!store.deleted(&video_key()).unwrap().unwrap().1);
    assert!(second.pending_projection().unwrap() > 0);
    store.finish_delete(&video_key(), None).unwrap();
    second.project_pending().unwrap();
    assert!(store.deleted(&video_key()).unwrap().is_none());
    assert_eq!(
        store.cues(&video_key()).unwrap()[0].text_ko.as_deref(),
        Some("Saved translation")
    );
}

#[test]
fn invalid_legacy_completion_rolls_back_the_copy_and_active_jobs_stay_local() {
    let source = Profile::new();
    let target = Profile::new();
    let store = Store::open(&source.0.join("app.sqlite3")).unwrap();
    let Change::Translation { translation: t } = translation() else {
        unreachable!()
    };
    store
        .create_job(
            "unfinished",
            &t.video.key,
            &t.video.page_url,
            "other",
            &t.video.title,
            t.duration_ms,
            None,
            &t.model,
        )
        .unwrap();
    let journal = target.journal();
    assert_eq!(journal.import_legacy(&store).unwrap(), 1);
    assert!(journal.library().unwrap().active_job().unwrap().is_none());
    assert!(journal
        .library()
        .unwrap()
        .cues(&t.video.key)
        .unwrap()
        .is_empty());
    let db = Db::open(store.path()).unwrap();
    db.execute("UPDATE job SET stage='ready' WHERE id='unfinished'", &[])
        .unwrap();
    let snapshot = journal.records().unwrap();
    assert!(journal.import_legacy(&store).is_err());
    assert_eq!(journal.records().unwrap(), snapshot);
    assert_eq!(
        store.ready_job(&t.video.key).unwrap().unwrap().id,
        "unfinished"
    );
}
