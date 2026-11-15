use super::*;

struct Profile(PathBuf);
impl Profile {
    fn new() -> Self {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        Self(std::env::temp_dir().join(format!("vto-sync-{}", digest(&random))))
    }
    fn journal(&self, subject: &str) -> Journal {
        Journal::open(
            &self.0,
            &crate::google_auth::VerifiedIdentity::fixture(subject),
        )
        .unwrap()
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn video() -> Video {
    let page_url = "https://lectures.example.org/one";
    Video {
        key: crate::api::video_key(&url::Url::parse(page_url).unwrap()),
        page_url: page_url.into(),
        title: "Saved lecture".into(),
    }
}

fn translation(text_ko: &str) -> Change {
    Change::Translation {
        translation: Translation {
            video: video(),
            duration_ms: 5000,
            source_language: Some("en".into()),
            terms: vec![Term {
                source: "attention".into(),
                target: "translated term".into(),
            }],
            model: crate::gemini::MODEL.into(),
            cost_krw: 12.5,
            cues: vec![Cue {
                start_ms: 1234,
                end_ms: 4321,
                source_text: "Original speech".into(),
                text_ko: text_ko.into(),
            }],
        },
    }
}

fn deliver(from: &Journal, to: &Journal) {
    loop {
        let after = to.cursor(from.replica()).unwrap();
        let page = from.page(after).unwrap();
        let wire = page.encode().unwrap();
        to.receive(from.replica(), &Page::decode(&wire).unwrap())
            .unwrap();
        if page.through == after {
            break;
        }
    }
}

fn state(journal: &Journal) -> BTreeMap<String, String> {
    journal
        .records()
        .unwrap()
        .iter()
        .map(|r| (r.identity(), serde_json::to_string(r).unwrap()))
        .collect()
}

#[test]
fn completed_data_and_transfer_cursors_survive_restart_in_separate_accounts() {
    let first = Profile::new();
    let second = Profile::new();
    let a = first.journal("google-user-one");
    let b = second.journal("google-user-one");
    let original = a
        .write(translation("Saved translation"), Deletions::new())
        .unwrap();
    let replica = a.replica().to_string();
    drop(a);
    let a = first.journal("google-user-one");
    assert_eq!(a.replica(), replica);
    let page = a.page(0).unwrap();
    assert_eq!(page.records[0].record, original);
    assert!(b.receive(a.replica(), &page).unwrap());
    let checkpoint = page.through;
    drop(b);
    let b = second.journal("google-user-one");
    assert_eq!(b.cursor(a.replica()).unwrap(), checkpoint);
    assert!(!b.receive(a.replica(), &page).unwrap());
    assert_eq!(b.records().unwrap(), vec![original]);

    let other = second.journal("google-user-two");
    assert_ne!(other.account(), b.account());
    assert_ne!(other.path, b.path);
    assert!(other.receive(a.replica(), &page).is_err());
    assert!(other.records().unwrap().is_empty());
    assert_eq!(other.cursor(a.replica()).unwrap(), 0);
    assert!(b.receive(other.replica(), &page).is_err());
}

#[test]
fn concurrent_translations_and_independent_playlist_fields_converge_without_echoes() {
    let first = Profile::new();
    let second = Profile::new();
    let relay = Profile::new();
    let a = first.journal("same-account");
    let b = second.journal("same-account");
    let c = relay.journal("same-account");
    a.write(translation("Translation from Windows"), Deletions::new())
        .unwrap();
    b.write(translation("Translation from Android"), Deletions::new())
        .unwrap();
    for journal in [&a, &b] {
        journal
            .write(
                Change::PlaylistName {
                    id: "playlist".into(),
                    name: "Initial name".into(),
                },
                Deletions::new(),
            )
            .unwrap();
    }
    a.write(
        Change::PlaylistName {
            id: "playlist".into(),
            name: "New name".into(),
        },
        Deletions::new(),
    )
    .unwrap();
    b.write(
        Change::PlaylistOrder {
            id: "playlist".into(),
            position: 5,
        },
        Deletions::new(),
    )
    .unwrap();
    for _ in 0..3 {
        deliver(&a, &c);
        deliver(&b, &c);
        deliver(&c, &a);
        deliver(&c, &b);
    }
    assert_eq!(state(&a), state(&b));
    assert_eq!(state(&b), state(&c));
    let records = a.records().unwrap();
    assert_eq!(
        records
            .iter()
            .filter(|r| matches!(r.change, Change::Translation { .. }))
            .count(),
        2
    );
    assert!(records
        .iter()
        .any(|r| matches!(&r.change, Change::PlaylistName { name, .. } if name=="New name")));
    assert!(records
        .iter()
        .any(|r| matches!(&r.change, Change::PlaylistOrder { position: 5, .. })));
    let before = [
        a.page(0).unwrap().through,
        b.page(0).unwrap().through,
        c.page(0).unwrap().through,
    ];
    deliver(&a, &c);
    deliver(&b, &c);
    deliver(&c, &a);
    deliver(&c, &b);
    assert_eq!(
        before,
        [
            a.page(0).unwrap().through,
            b.page(0).unwrap().through,
            c.page(0).unwrap().through
        ]
    );
}

#[test]
fn disconnected_edits_cannot_resurrect_deleted_data_even_with_a_higher_clock() {
    let first = Profile::new();
    let second = Profile::new();
    let a = first.journal("same-account");
    let b = second.journal("same-account");
    a.write(translation("Original translation"), Deletions::new())
        .unwrap();
    a.write(
        Change::EntryPresence {
            id: "playlist".into(),
            key: video().key,
            present: true,
        },
        Deletions::new(),
    )
    .unwrap();
    deliver(&a, &b);
    a.write(Change::DeleteVideo { key: video().key }, Deletions::new())
        .unwrap();
    for n in 0..10 {
        b.write(
            Change::Playback {
                key: video().key,
                position_ms: n,
            },
            Deletions::new(),
        )
        .unwrap();
    }
    b.write(translation("Offline replacement"), Deletions::new())
        .unwrap();
    deliver(&b, &a);
    assert!(a
        .records()
        .unwrap()
        .iter()
        .all(|r| matches!(r.change, Change::DeleteVideo { .. })));
    deliver(&a, &b);
    assert_eq!(state(&a), state(&b));
    assert!(b
        .write(translation("Stale worker"), Deletions::new())
        .is_err());
    let consent = b.deletions(&video().key).unwrap();
    b.write(translation("Confirmed new translation"), consent.clone())
        .unwrap();
    deliver(&b, &a);
    assert!(a
        .records()
        .unwrap()
        .iter()
        .any(|r| matches!(r.change, Change::Translation { .. })));
    a.write(Change::DeleteVideo { key: video().key }, Deletions::new())
        .unwrap();
    deliver(&a, &b);
    assert!(b.write(translation("Old confirmation"), consent).is_err());
    drop(a);
    let a = first.journal("same-account");
    assert!(a
        .records()
        .unwrap()
        .iter()
        .all(|r| matches!(r.change, Change::DeleteVideo { .. })));
    assert!(!a
        .page(0)
        .unwrap()
        .encode()
        .unwrap()
        .windows("Original speech".len())
        .any(|v| v == b"Original speech"));
}

#[test]
fn concurrent_deletions_require_confirmation_of_both_before_restoration() {
    let first = Profile::new();
    let second = Profile::new();
    let a = first.journal("same-account");
    let b = second.journal("same-account");
    a.write(Change::DeleteVideo { key: video().key }, Deletions::new())
        .unwrap();
    b.write(Change::DeleteVideo { key: video().key }, Deletions::new())
        .unwrap();
    let first_consent = a.deletions(&video().key).unwrap();
    a.write(
        translation("Restored before learning of other deletion"),
        first_consent,
    )
    .unwrap();
    deliver(&b, &a);
    deliver(&a, &b);
    assert_eq!(state(&a), state(&b));
    assert_eq!(a.deletions(&video().key).unwrap().len(), 2);
    assert!(a
        .records()
        .unwrap()
        .iter()
        .all(|r| matches!(r.change, Change::DeleteVideo { .. })));
    a.write(
        translation("Both deletions confirmed"),
        a.deletions(&video().key).unwrap(),
    )
    .unwrap();
    deliver(&a, &b);
    assert_eq!(state(&a), state(&b));
    assert!(b
        .records()
        .unwrap()
        .iter()
        .any(|r| matches!(r.change, Change::Translation { .. })));
}

#[test]
fn playlist_deletion_keeps_translations_and_rejects_stale_memberships() {
    let profile = Profile::new();
    let a = profile.journal("same-account");
    a.write(translation("Keep this"), Deletions::new()).unwrap();
    a.write(
        Change::PlaylistName {
            id: "topic".into(),
            name: "Topic".into(),
        },
        Deletions::new(),
    )
    .unwrap();
    a.write(
        Change::EntryPresence {
            id: "topic".into(),
            key: video().key,
            present: true,
        },
        Deletions::new(),
    )
    .unwrap();
    a.write(
        Change::DeletePlaylist { id: "topic".into() },
        Deletions::new(),
    )
    .unwrap();
    assert_eq!(a.records().unwrap().len(), 2);
    assert!(a
        .records()
        .unwrap()
        .iter()
        .any(|r| matches!(r.change, Change::Translation { .. })));
    assert!(a
        .write(
            Change::EntryPresence {
                id: "topic".into(),
                key: video().key,
                present: true
            },
            Deletions::new()
        )
        .is_err());
}

#[test]
fn interrupted_or_conflicting_pages_never_advance_the_cursor_or_leave_partial_rows() {
    let first = Profile::new();
    let second = Profile::new();
    let a = first.journal("same-account");
    let b = second.journal("same-account");
    a.write(
        Change::PlaylistName {
            id: "topic".into(),
            name: "Topic".into(),
        },
        Deletions::new(),
    )
    .unwrap();
    let good = a.page(0).unwrap();
    let bytes = good.encode().unwrap();
    assert!(Page::decode(&bytes[..bytes.len() - 1]).is_err());
    let mut conflicting = good.clone();
    conflicting.records.push(conflicting.records[0].clone());
    conflicting.records[1].sequence = 2;
    conflicting.records[1].record.change = Change::PlaylistName {
        id: "different-id".into(),
        name: "Altered content with the same event stamp".into(),
    };
    conflicting.through = 2;
    assert!(b.receive(a.replica(), &conflicting).is_err());
    assert!(b.records().unwrap().is_empty());
    assert_eq!(b.cursor(a.replica()).unwrap(), 0);
    assert!(b.receive(a.replica(), &good).unwrap());
    assert!(!b.receive(a.replica(), &good).unwrap());
    a.write(
        Change::PlaylistOrder {
            id: "topic".into(),
            position: 2,
        },
        Deletions::new(),
    )
    .unwrap();
    let mut gap = a.page(1).unwrap();
    gap.after = 2;
    gap.through = 3;
    gap.records[0].sequence = 3;
    assert!(b.receive(a.replica(), &gap).is_err());
    assert_eq!(b.cursor(a.replica()).unwrap(), 1);
}

#[test]
fn pagination_is_bounded_and_each_checkpoint_is_durable() {
    let first = Profile::new();
    let second = Profile::new();
    let a = first.journal("same-account");
    let b = second.journal("same-account");
    for i in 0..MAX_PAGE_RECORDS + 3 {
        a.write(
            Change::PlaylistName {
                id: format!("p{i}"),
                name: format!("Topic {i}"),
            },
            Deletions::new(),
        )
        .unwrap();
    }
    let page = a.page(0).unwrap();
    assert_eq!(page.records.len(), MAX_PAGE_RECORDS);
    b.receive(a.replica(), &page).unwrap();
    drop(b);
    let b = second.journal("same-account");
    let next = a.page(b.cursor(a.replica()).unwrap()).unwrap();
    assert_eq!(next.records.len(), 3);
    b.receive(a.replica(), &next).unwrap();
    assert_eq!(state(&a), state(&b));
    assert!(a.page(next.through + 1).is_err());
}

#[test]
fn transfer_validation_rejects_files_credentials_incomplete_cues_and_unbounded_values() {
    let profile = Profile::new();
    let journal = profile.journal("same-account");
    journal
        .write(translation("Translation"), Deletions::new())
        .unwrap();
    let good = journal.page(0).unwrap();
    for field in [
        "gemini_api_key",
        "cookies",
        "audio_path",
        "refresh_token",
        "job",
    ] {
        let mut value = serde_json::to_value(&good).unwrap();
        value["records"][0]["record"]["change"][field] = "not permitted".into();
        assert!(
            Page::decode(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{field}"
        );
    }
    for invalid in [
        "file:///private/file",
        "https://user:password@example.org/a",
        "javascript:alert(1)",
    ] {
        let mut page = good.clone();
        let Change::Translation { translation } = &mut page.records[0].record.change else {
            unreachable!()
        };
        translation.video.page_url = invalid.into();
        assert!(page.encode().is_err(), "{invalid}");
    }
    for case in 0..6 {
        let mut page = good.clone();
        let Change::Translation { translation } = &mut page.records[0].record.change else {
            unreachable!()
        };
        match case {
            0 => translation.cues[0].text_ko.clear(),
            1 => translation.cues[0].start_ms = -1,
            2 => translation.cues[0].end_ms = 5001,
            3 => translation.cues.push(translation.cues[0].clone()),
            4 => translation.cost_krw = f64::INFINITY,
            _ => translation.cues[0].source_text = "x".repeat(16_385),
        }
        assert!(page.encode().is_err(), "case {case}");
    }
    assert!(Page::decode(&vec![b' '; MAX_PAGE_BYTES + 1]).is_err());
    let mut page = good;
    page.records[0].record.stamp.clock = i64::MAX;
    assert!(page.encode().is_err());
}

#[test]
fn wire_roundtrips_preserve_recorded_cost_and_content_identity() {
    let record = Record {
        stamp: Stamp {
            clock: 1,
            replica: "a".repeat(32),
        },
        seen_deletions: Deletions::new(),
        change: translation("Translation"),
    };
    for i in 0..4096 {
        let mut original = record.clone();
        let Change::Translation { translation } = &mut original.change else {
            unreachable!()
        };
        translation.cost_krw = (i as f64 + 0.123456789012345) * 0.000123456789012345;
        let bytes = serde_json::to_vec(&original).unwrap();
        let decoded: Record = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(original, decoded, "cost sample {i}");
        assert_eq!(
            original.identity(),
            decoded.identity(),
            "identity sample {i}"
        );
    }
}

#[test]
fn simultaneous_local_writers_allocate_distinct_durable_stamps() {
    let profile = Profile::new();
    let a = std::sync::Arc::new(profile.journal("same-account"));
    let start = std::sync::Arc::new(std::sync::Barrier::new(3));
    let writers: Vec<_> = (0..2)
        .map(|n| {
            let a = a.clone();
            let start = start.clone();
            std::thread::spawn(move || {
                start.wait();
                a.write(
                    Change::PlaylistName {
                        id: format!("playlist-{n}"),
                        name: format!("Topic {n}"),
                    },
                    Deletions::new(),
                )
                .unwrap()
                .stamp
            })
        })
        .collect();
    start.wait();
    let mut clocks: Vec<_> = writers
        .into_iter()
        .map(|t| t.join().unwrap().clock)
        .collect();
    clocks.sort();
    assert_eq!(clocks, [1, 2]);
    assert_eq!(a.records().unwrap().len(), 2);
    assert_eq!(a.page(0).unwrap().through, 2);
}
