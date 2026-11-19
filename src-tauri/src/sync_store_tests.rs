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
fn silent_cue_repair_is_atomic_retains_versions_cost_and_sync_selection() {
    let a = Profile::new();
    let b = Profile::new();
    let first = a.journal();
    let second = b.journal();
    let Change::Translation { mut translation } = translation() else { unreachable!() };
    let key = translation.video.key.clone();
    translation.duration_ms = 20000;
    translation.cost_krw = 276.293875;
    translation.cues = vec![
        Cue { start_ms:1180, end_ms:5800, source_text:format!("[♪ {}", "Music playing in background, ".repeat(42)), text_ko:"Retained music description".into() },
        Cue { start_ms:9100, end_ms:18100, source_text:"Good afternoon everyone".into(), text_ko:"Saved speech translation".into() },
    ];
    let previous = first.write(Change::Translation { translation:translation.clone() }, Deletions::new()).unwrap().identity();
    for change in [Change::PlaylistName {id:"topic".into(),name:"Saved topic".into()},
        Change::EntryPresence {id:"topic".into(),key:key.clone(),present:true},
        Change::Playback {key:key.clone(),position_ms:12000}] {
        first.write(change, Deletions::new()).unwrap();
    }
    let mut unrelated = translation.clone();
    unrelated.video.page_url = "https://lectures.example.org/unrelated".into();
    unrelated.video.key = crate::api::video_key(&url::Url::parse(&unrelated.video.page_url).unwrap());
    let other_key = unrelated.video.key.clone();
    first.write(Change::Translation {translation:unrelated}, Deletions::new()).unwrap();
    let store = first.library().unwrap();
    let before = store.cues(&key).unwrap();
    let untouched = store.cues(&other_key).unwrap();
    let before_library = store.library().unwrap();
    let db = Db::open(store.path()).unwrap();
    db.execute("UPDATE job SET cost_krw=320 WHERE video_key=?", p![&key]).unwrap();
    let inspected = store.inspect_silent_cue(&key, &previous, 0).unwrap();
    let mut unavailable = inspected.clone();
    unavailable.seq = 99;
    assert!(store.repair_silent_cue(&key, &previous, &unavailable).is_err());
    assert_eq!(store.cues(&key).unwrap()[0].source_text, before[0].source_text);

    // A lost process between publication and commit leaves neither a partial selection nor cues.
    {
        let db = Db::open(store.path()).unwrap();
        db.exec("BEGIN IMMEDIATE").unwrap();
        local::repair_silent_cue(&db, &key, &previous, &inspected).unwrap();
    }
    assert_eq!(store.cues(&key).unwrap().len(), 2);
    assert!(store.library().unwrap()["videos"].as_array().unwrap().iter().find(|v|v["video_key"]==key).unwrap()["versions"].as_array().unwrap().iter().any(|v|v["id"]==previous && v["selected"]==true));

    db.execute("CREATE TRIGGER reject_repair BEFORE INSERT ON cue BEGIN SELECT RAISE(ABORT,'repair write failure'); END", &[]).unwrap();
    assert!(store.repair_silent_cue(&key, &previous, &inspected).is_err());
    db.exec("DROP TRIGGER reject_repair").unwrap();
    assert_eq!(store.cues(&key).unwrap().len(), 2);
    let replacement = store.repair_silent_cue(&key, &previous, &inspected).unwrap();
    assert_ne!(replacement, previous);
    assert!(store.repair_silent_cue(&key, &previous, &inspected).is_err(), "stale selection must not remove speech");
    let repaired = store.cues(&key).unwrap();
    assert_eq!(repaired.len(), 1);
    assert_eq!((repaired[0].start_ms, repaired[0].end_ms), (9100,18100));
    assert_eq!(repaired[0].source_text, before[1].source_text);
    assert_eq!(repaired[0].text_ko, before[1].text_ko);
    assert_eq!(store.ready_job(&key).unwrap().unwrap().cost_krw,320.0);
    assert_eq!(store.playback(&key).unwrap(),Some(12000));
    assert_eq!(store.library().unwrap()["playlists"],before_library["playlists"]);
    assert_eq!(store.cues(&other_key).unwrap()[0].source_text,untouched[0].source_text);
    send(&first,&second);
    // Receiving the same old response again cannot select it over the repaired version.
    send(&first,&second);
    let reopened = second.library().unwrap();
    assert_eq!(reopened.cues(&key).unwrap().len(),1);
    assert_eq!(reopened.ready_job(&key).unwrap().unwrap().cost_krw,320.0);
    let versions = reopened.library().unwrap();
    let versions = versions["videos"].as_array().unwrap().iter().find(|v|v["video_key"]==key).unwrap()["versions"].as_array().unwrap();
    assert_eq!(versions.len(),2);
    assert!(versions.iter().any(|v|v["id"]==replacement && v["selected"]==true));
    reopened.select_translation(&key,&previous).unwrap();
    assert_eq!(reopened.cues(&key).unwrap().len(),2);
    assert_eq!(reopened.retranslation_cost(&key).unwrap(),320.0);
    let other_account = Journal::open(&a.0,&VerifiedIdentity::fixture("another-account")).unwrap();
    assert!(other_account.library().unwrap().repair_silent_cue(&key,&previous,&inspected).is_err());
}

#[test]
fn silent_cue_repair_denies_any_change_to_the_inspected_selection_or_cue() {
    let profile = Profile::new();
    let journal = profile.journal();
    let Change::Translation { mut translation } = translation() else { unreachable!() };
    let key = translation.video.key.clone();
    translation.duration_ms = 20000;
    translation.cues = vec![
        Cue {start_ms:9100,end_ms:18100,source_text:"Good afternoon everyone".into(),text_ko:"Saved speech".into()},
        Cue {start_ms:18200,end_ms:19000,source_text:"Welcome".into(),text_ko:"Saved welcome".into()},
    ];
    let speech = journal.write(Change::Translation {translation:translation.clone()}, Deletions::new()).unwrap().identity();
    translation.cues[0] = Cue {start_ms:1180,end_ms:5800,source_text:"Music description".into(),text_ko:"Saved music".into()};
    let music = journal.write(Change::Translation {translation}, Deletions::new()).unwrap().identity();
    let store = journal.library().unwrap();
    // A requested version must be selected at inspection, not merely at publication.
    assert!(store.inspect_silent_cue(&key,&speech,0).is_err());
    let inspected_music = store.inspect_silent_cue(&key,&music,0).unwrap();
    store.select_translation(&key,&speech).unwrap();
    let before = store.cues(&key).unwrap();
    assert!(store.repair_silent_cue(&key,&speech,&inspected_music).is_err());
    assert!(store.repair_silent_cue(&key,&music,&inspected_music).is_err());
    // Every component of the audio/text inspection is bound, including missing translations.
    for field in ["sequence","start","end","source","translation","missing translation"] {
        let mut changed = store.inspect_silent_cue(&key,&speech,0).unwrap();
        match field {
            "sequence" => changed.seq = 1,
            "start" => changed.start_ms += 1,
            "end" => changed.end_ms += 1,
            "source" => changed.source_text.push('!'),
            "translation" => changed.text_ko.as_mut().unwrap().push('!'),
            _ => changed.text_ko = None,
        }
        assert!(store.repair_silent_cue(&key,&speech,&changed).is_err(), "{field}");
        assert_eq!(store.cues(&key).unwrap(),before,"{field}");
    }
    assert_eq!(store.library().unwrap()["videos"][0]["versions"].as_array().unwrap().len(),2);
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
    store.save_playback(&t.video.key, 2345, 5000).unwrap();
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
    assert!(journal.has_legacy_changes(&store).unwrap());
    assert!(journal.import_legacy(&store).unwrap() > 0);
    assert!(!journal.has_legacy_changes(&store).unwrap());
    assert_eq!(store.library().unwrap(), original);
    assert_eq!(
        store.ready_job(&t.video.key).unwrap().unwrap().id,
        "legacy-ready"
    );
    assert_eq!(journal.import_legacy(&store).unwrap(), 0);
    let copied = journal.library().unwrap();
    assert_eq!(copied.playback(&t.video.key).unwrap(), Some(2345));
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
    assert!(other.has_legacy_changes(&copied).is_err());
    journal
        .write(
            Change::DeleteVideo {
                key: t.video.key.clone(),
            },
            Deletions::new(),
        )
        .unwrap();
    assert_eq!(journal.import_legacy(&store).unwrap(), 0);
    store.create_playlist("later", "Added after first import").unwrap();
    assert!(journal.has_legacy_changes(&store).unwrap());
    assert_eq!(journal.import_legacy(&store).unwrap(), 2);
    assert!(!journal.has_legacy_changes(&store).unwrap());
    assert_eq!(journal.library().unwrap().library().unwrap()["playlists"].as_array().unwrap().len(), 2);
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
    alternate.cost_krw = 18.5;
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
    let versions = first.records().unwrap().into_iter().filter(|r| matches!(r.change, Change::Translation { .. })).collect::<Vec<_>>();
    for version in &versions {
        let store = first.library().unwrap();
        let Change::Translation { translation: expected } = &version.change else { unreachable!() };
        store.select_translation(&video_key(), &version.identity()).unwrap();
        let through = first.page(0).unwrap().through;
        store.select_translation(&video_key(), &version.identity()).unwrap();
        assert_eq!(first.page(0).unwrap().through, through);
        send(&first, &second);
        for journal in [&first, &second] {
            let reopened = journal.library().unwrap();
            assert_eq!(reopened.cues(&video_key()).unwrap()[0].text_ko.as_deref(), Some(expected.cues[0].text_ko.as_str()));
            assert_eq!(reopened.ready_job(&video_key()).unwrap().unwrap().cost_krw, expected.cost_krw);
            assert_eq!(reopened.latest_job(&video_key()).unwrap().unwrap().cost_krw, expected.cost_krw);
            let library = reopened.library().unwrap();
            let listed = library["videos"][0]["versions"].as_array().unwrap();
            assert_eq!(listed.len(), 2);
            assert_eq!(listed.iter().filter(|v| v["selected"]==true).count(), 1);
        }
    }
    let original = versions.iter().find(|r| matches!(&r.change, Change::Translation { translation } if translation.cost_krw==12.5)).unwrap();
    first.library().unwrap().select_translation(&video_key(), &original.identity()).unwrap();
    let fresh = Profile::new();
    let third = fresh.journal();
    send(&first, &third);
    let fresh_store = third.library().unwrap();
    let old_snapshot = fresh_store.ready_job(&video_key()).unwrap().unwrap();
    assert_eq!(old_snapshot.cost_krw, 12.5);
    assert_eq!(fresh_store.retranslation_cost(&video_key()).unwrap(), 18.5);
    fresh_store.retranslate("budget-first", &old_snapshot, crate::gemini::MODEL).unwrap();
    assert_eq!(fresh_store.job("budget-first").unwrap().unwrap().cost_krw, 18.5);
    assert!(fresh_store.add_usage("budget-first", -1, 1, |_, _| 2.0).is_err());
    assert!(fresh_store.add_usage("budget-first", 1, 1, |_, _| f64::NAN).is_err());
    fresh_store.add_usage("budget-first", 1, 1, |_, _| 2.0).unwrap();
    assert!(fresh_store.add_usage("budget-first", i64::MAX, 1, |_, _| 2.0).is_err());
    fresh_store.fail("budget-first", "fixture_finished").unwrap();
    fresh_store.retranslate("budget-next", &old_snapshot, crate::gemini::MODEL).unwrap();
    assert_eq!(fresh_store.job("budget-next").unwrap().unwrap().cost_krw, 20.5);
    assert!(first.library().unwrap().select_translation(&video_key(), "t/not-owned").is_err());
    let selected = versions[0].identity();
    let store = first.library().unwrap();
    store.create_job("active-version", &video_key(), "https://lectures.example.org/projected", "other", "Active", 5000, None, crate::gemini::MODEL).unwrap();
    assert!(store.select_translation(&video_key(), &selected).is_err());
    store.fail("active-version", "fixture_finished").unwrap();
    first
        .write(Change::DeleteVideo { key: video_key() }, Deletions::new())
        .unwrap();
    assert!(store.select_translation(&video_key(), &selected).is_err());
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

fn prepare_local_job(store: &Store, id: &str) {
    let Change::Translation { translation: t } = translation() else {
        unreachable!()
    };
    let consent = store.deleted(&t.video.key).unwrap().map(|(token, _)| token);
    store
        .create_job_with_consent(
            id,
            &t.video.key,
            &t.video.page_url,
            "other",
            &t.video.title,
            t.duration_ms,
            None,
            &t.model,
            consent.as_deref(),
        )
        .unwrap();
    let db = Db::open(store.path()).unwrap();
    db.execute(
        "UPDATE job SET stage='translating',cost_krw=12.5 WHERE id=?",
        p![id],
    )
    .unwrap();
    db.execute(
        "INSERT INTO cue VALUES (?,?,?,?,?,?)",
        p![
            t.video.key,
            0i64,
            1234i64,
            4321i64,
            "Original source",
            "Saved translation"
        ],
    )
    .unwrap();
}

#[test]
fn native_completion_and_its_journal_commit_atomically_and_do_not_replace_the_local_job() {
    let a = Profile::new();
    let b = Profile::new();
    let first = a.journal();
    let second = b.journal();
    let store = first.library().unwrap();
    prepare_local_job(&store, "native-complete");
    let db = Db::open(store.path()).unwrap();
    db.execute("CREATE TRIGGER refuse_journal BEFORE INSERT ON sync_seen BEGIN SELECT RAISE(ABORT,'fixture journal failure'); END", &[]).unwrap();
    assert!(store.finish("native-complete").is_err());
    assert_eq!(
        store.job("native-complete").unwrap().unwrap().stage,
        "translating"
    );
    assert!(first.records().unwrap().is_empty());
    db.execute("DROP TRIGGER refuse_journal", &[]).unwrap();
    store.finish("native-complete").unwrap();
    first.project_pending().unwrap();
    assert_eq!(
        store.ready_job(&video_key()).unwrap().unwrap().id,
        "native-complete"
    );
    send(&first, &second);
    assert_eq!(
        second.library().unwrap().cues(&video_key()).unwrap(),
        store.cues(&video_key()).unwrap()
    );
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
    let previous = store.ready_job(&video_key()).unwrap().unwrap();
    store
        .retranslate("native-retranslation", &previous, crate::gemini::MODEL)
        .unwrap();
    assert!(store.finish("native-retranslation").is_err());
    assert_eq!(
        first
            .records()
            .unwrap()
            .iter()
            .filter(|r| matches!(r.change, Change::Translation { .. }))
            .count(),
        1
    );
    store
        .set_translations(
            "native-retranslation",
            &[(0, "Complete replacement".into())],
        )
        .unwrap();
    store.finish("native-retranslation").unwrap();
    first.project_pending().unwrap();
    assert_eq!(
        store.ready_job(&video_key()).unwrap().unwrap().id,
        "native-retranslation"
    );
    send(&first, &second);
    assert_eq!(
        second.library().unwrap().cues(&video_key()).unwrap()[0]
            .text_ko
            .as_deref(),
        Some("Complete replacement")
    );
    assert_eq!(
        first
            .records()
            .unwrap()
            .iter()
            .filter(|r| matches!(r.change, Change::Translation { .. }))
            .count(),
        2
    );
}

#[test]
fn ordinary_library_mutations_publish_and_incoming_projection_does_not_echo() {
    let a = Profile::new();
    let b = Profile::new();
    let first = a.journal();
    let second = b.journal();
    let store = first.library().unwrap();
    let Change::Translation { translation: t } = translation() else {
        unreachable!()
    };
    store.create_playlist("one", "First").unwrap();
    store.create_playlist("two", "Second").unwrap();
    store.rename_playlist("one", "Renamed").unwrap();
    store.move_library_item("two", None, -1).unwrap();
    store
        .add_entry(
            "one",
            &video_key(),
            &t.video.page_url,
            "other",
            &t.video.title,
        )
        .unwrap();
    send(&first, &second);
    assert_eq!(
        store.library().unwrap(),
        second.library().unwrap().library().unwrap()
    );
    let checkpoint = second.page(0).unwrap().through;
    second.project_pending().unwrap();
    send(&first, &second);
    assert_eq!(second.page(0).unwrap().through, checkpoint);
    store.remove_entry("one", &video_key()).unwrap();
    store.delete_playlist("two").unwrap();
    send(&first, &second);
    assert_eq!(
        store.library().unwrap(),
        second.library().unwrap().library().unwrap()
    );
    store.begin_delete(&video_key(), "first-cleanup").unwrap();
    let after_delete = first.page(0).unwrap().through;
    store.begin_delete(&video_key(), "retry-cleanup").unwrap();
    assert_eq!(first.page(0).unwrap().through, after_delete);
}

#[test]
fn multi_place_moves_reach_the_peer_in_the_same_order() {
    let a = Profile::new();
    let b = Profile::new();
    let first = a.journal();
    let second = b.journal();
    let store = first.library().unwrap();
    for id in ["one", "two", "three"] {
        store.create_playlist(id, id).unwrap();
    }
    let keys: Vec<String> = (0..3)
        .map(|n| {
            let page = format!("https://lectures.example.org/{n}");
            let key = crate::api::video_key(&url::Url::parse(&page).unwrap());
            store.add_entry("one", &key, &page, "other", &format!("Lecture {n}")).unwrap();
            key
        })
        .collect();
    send(&first, &second);
    store.move_library_item("three", None, -2).unwrap();
    store.move_library_item("one", Some(&keys[0]), 2).unwrap();
    send(&first, &second);
    let mine = store.library().unwrap();
    let ids: Vec<&str> = mine["playlists"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["three", "one", "two"]);
    assert_eq!(mine["playlists"][1]["entries"], serde_json::json!([keys[1], keys[2], keys[0]]));
    assert_eq!(mine, second.library().unwrap().library().unwrap());
}

#[test]
fn job_completion_cannot_adopt_a_deletion_received_after_the_job_started() {
    let a = Profile::new();
    let b = Profile::new();
    let first = a.journal();
    let second = b.journal();
    let store = second.library().unwrap();
    prepare_local_job(&store, "started-before-delete");
    first
        .write(Change::DeleteVideo { key: video_key() }, Deletions::new())
        .unwrap();
    send(&first, &second);
    drop(store);
    drop(second);
    let second = b.journal();
    let store = second.library().unwrap();
    store.finish("started-before-delete").unwrap();
    second.project_pending().unwrap();
    assert!(second
        .records()
        .unwrap()
        .iter()
        .all(|r| !matches!(r.change, Change::Translation { .. })));
    assert!(store.deleted(&video_key()).unwrap().is_some());
    assert_eq!(store.library().unwrap()["videos"][0]["saved"], false);
    store.finish_delete(&video_key(), None).unwrap();
    // The API admits this new job only after checking the completed marker's confirmation token.
    prepare_local_job(&store, "newly-confirmed-job");
    store.finish("newly-confirmed-job").unwrap();
    second.project_pending().unwrap();
    assert!(store.deleted(&video_key()).unwrap().is_none());
    assert_eq!(
        store.ready_job(&video_key()).unwrap().unwrap().id,
        "newly-confirmed-job"
    );
}

#[test]
fn concurrent_order_ties_have_the_same_visible_order_on_both_replicas() {
    let a = Profile::new();
    let b = Profile::new();
    let first = a.journal();
    let second = b.journal();
    first
        .library()
        .unwrap()
        .create_playlist("z-first", "From first")
        .unwrap();
    second
        .library()
        .unwrap()
        .create_playlist("a-second", "From second")
        .unwrap();
    send(&first, &second);
    send(&second, &first);
    assert_eq!(
        first.library().unwrap().library().unwrap(),
        second.library().unwrap().library().unwrap()
    );
    assert_eq!(
        first.library().unwrap().library().unwrap()["playlists"][0]["id"],
        "a-second"
    );
}

#[test]
fn queued_remote_deletion_invalidates_earlier_consent_before_projection() {
    let a = Profile::new();
    let b = Profile::new();
    let first = a.journal();
    let second = b.journal();
    first
        .write(Change::DeleteVideo { key: video_key() }, Deletions::new())
        .unwrap();
    send(&first, &second);
    let store = second.library().unwrap();
    store.finish_delete(&video_key(), None).unwrap();
    let old_token = store.deleted(&video_key()).unwrap().unwrap().0;
    let deletion = first
        .write(Change::DeleteVideo { key: video_key() }, Deletions::new())
        .unwrap();
    second.transaction(|db| merge(db, &deletion)).unwrap();
    let Change::Translation { translation: t } = translation() else {
        unreachable!()
    };
    assert!(store
        .create_job_with_consent(
            "stale-consent",
            &t.video.key,
            &t.video.page_url,
            "other",
            &t.video.title,
            t.duration_ms,
            None,
            &t.model,
            Some(&old_token)
        )
        .is_err());
    assert!(store.active_job().unwrap().is_none());
    second.project_pending().unwrap();
    assert!(!store.deleted(&video_key()).unwrap().unwrap().1);
    store.finish_delete(&video_key(), None).unwrap();
    let new_token = store.deleted(&video_key()).unwrap().unwrap().0;
    assert_ne!(old_token, new_token);
    second.project_pending().unwrap();
    assert!(store.deleted(&video_key()).unwrap().unwrap().1);
    assert!(store
        .create_job_with_consent(
            "old-token",
            &t.video.key,
            &t.video.page_url,
            "other",
            &t.video.title,
            t.duration_ms,
            None,
            &t.model,
            Some(&old_token)
        )
        .is_err());
    store
        .create_job_with_consent(
            "fresh-consent",
            &t.video.key,
            &t.video.page_url,
            "other",
            &t.video.title,
            t.duration_ms,
            None,
            &t.model,
            Some(&new_token),
        )
        .unwrap();
}

#[test]
fn deletion_and_a_fresh_link_or_translation_in_one_page_cannot_bypass_cleanup() {
    for restore_translation in [false, true] {
        let a = Profile::new();
        let b = Profile::new();
        let first = a.journal();
        let second = b.journal();
        first.write(translation(), Deletions::new()).unwrap();
        send(&first, &second);
        first
            .write(Change::DeleteVideo { key: video_key() }, Deletions::new())
            .unwrap();
        first
            .library()
            .unwrap()
            .finish_delete(&video_key(), None)
            .unwrap();
        let Change::Translation { translation: t } = translation() else {
            unreachable!()
        };
        let restored = if restore_translation {
            Change::Translation { translation: t }
        } else {
            Change::Video { video: t.video }
        };
        first
            .write(restored, first.deletions(&video_key()).unwrap())
            .unwrap();
        first.write(Change::Playback { key: video_key(), position_ms: 2222 }, first.deletions(&video_key()).unwrap()).unwrap();
        send(&first, &second);
        let store = second.library().unwrap();
        assert!(
            store
                .deleted(&video_key())
                .unwrap()
                .is_some_and(|(_, complete)| !complete),
            "incoming restoration skipped local cleanup"
        );
        assert_eq!(store.library().unwrap()["videos"][0]["saved"], false);
        store.finish_delete(&video_key(), None).unwrap();
        second.project_pending().unwrap();
        assert_eq!(store.playback(&video_key()).unwrap(), Some(2222));
        assert_eq!(
            store.ready_job(&video_key()).unwrap().is_some(),
            restore_translation
        );
        if !restore_translation {
            assert!(store.deleted(&video_key()).unwrap().unwrap().1);
            assert!(store.cues(&video_key()).unwrap().is_empty());
        }
    }
}

#[test]
fn a_failed_remake_continues_at_translating_or_projects_its_previous_version_again() {
    let profile = Profile::new();
    let journal = profile.journal();
    let Change::Translation { translation } = translation() else { unreachable!() };
    let key = translation.video.key.clone();
    let previous = journal.write(Change::Translation { translation }, Deletions::new()).unwrap().identity();
    let store = journal.library().unwrap();
    let before = store.cues(&key).unwrap();
    assert_eq!(store.begin_remake("remake-x", &key, "a.m4a", "m").unwrap(), previous);
    Db::open(store.path()).unwrap().execute("INSERT INTO cue VALUES (?,0,0,900,'Written again.',NULL)", p![key.as_str()]).unwrap();
    store.fail("remake-x", "translate_failed").unwrap();
    // Selecting the version that is still selected would leave the written cues.
    store.restore_translation(&key, &previous).unwrap();
    assert_eq!(store.cues(&key).unwrap(), before);
    assert!(store.library().unwrap()["videos"][0]["versions"].as_array().unwrap().iter().any(|v| v["id"] == previous && v["selected"] == true));
    // Continued after that restore, the re-make is the newest job again.
    std::thread::sleep(std::time::Duration::from_millis(5));
    store.continue_remake("remake-x").unwrap();
    let job = store.job("remake-x").unwrap().unwrap();
    assert_eq!((job.stage.as_str(), job.failure_reason), ("translating", None));
    assert_eq!(store.latest_job(&key).unwrap().unwrap().id, "remake-x");
    assert!(store.continue_remake("remake-x").is_err(), "only a failed re-make continues");
}
