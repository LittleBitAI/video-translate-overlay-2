use super::*;
use crate::{google_auth::VerifiedIdentity, sync::{Change, Deletions, Journal}};
use std::path::PathBuf;

struct Profile(PathBuf);
impl Profile {
    fn new() -> Self {
        let mut bytes = [0u8; 16]; getrandom::fill(&mut bytes).unwrap();
        let name: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let path = std::env::temp_dir().join(format!("vto-sync-cleanup-{name}"));
        std::fs::create_dir_all(path.join("audio")).unwrap(); Self(path)
    }
    fn core(&self) -> Core {
        let mut core = crate::pipeline::worker_tests::idle_core(&self.0, "");
        core.account = crate::account::tests::signed_in_manager(&self.0, "cleanup-account"); core
    }
}
impl Drop for Profile { fn drop(&mut self) { std::fs::remove_dir_all(&self.0).unwrap(); } }
fn key(label: &str) -> String { api::video_key(&url::Url::parse(&format!("https://lectures.example.org/{label}")).unwrap()) }
fn remove(remote: &Journal, owned: &Journal, key: &str) {
    remote.write(Change::DeleteVideo {key:key.into()}, Deletions::new()).unwrap();
    owned.receive(remote.replica(), &remote.page(owned.cursor(remote.replica()).unwrap()).unwrap()).unwrap();
}

#[test]
fn automatic_cleanup_removes_only_owned_files_and_handles_markers_without_visible_video_rows() {
    let profile = Profile::new(); let remote_profile = Profile::new();
    let core = profile.core(); let session = core.account.session().unwrap();
    let store = session.journal.library().unwrap();
    let remote = Journal::open(&remote_profile.0, &VerifiedIdentity::fixture("cleanup-account")).unwrap();
    let saved = key("saved"); let active = key("active");
    store.create_job("saved-job", &saved, "https://lectures.example.org/saved", "other", "Saved lecture", 5000, None, "m").unwrap();
    store.save_region("saved-job", &saved, 0, &[(123,2345,"original".into())], None).unwrap();
    store.set_translations("saved-job", &[(0,"translated".into())]).unwrap();
    store.set_terms(&saved, "[]").unwrap(); store.finish("saved-job").unwrap();
    store.create_playlist("topic", "Topic").unwrap();
    store.add_entry("topic", &saved, "https://lectures.example.org/saved", "other", "Saved lecture").unwrap();
    store.create_job("active-job", &active, "https://lectures.example.org/active", "other", "Active lecture", 5000, None, "m").unwrap();
    for file in ["saved-job.wav", "saved-job.wav.sections", "active-job.wav", "unrelated.wav"] {
        std::fs::write(profile.0.join("audio").join(file), b"fixture").unwrap();
    }
    std::fs::create_dir_all(profile.0.join("models")).unwrap();
    std::fs::write(profile.0.join("models/shared.bin"), b"shared model").unwrap();
    let original = core.store.library().unwrap();
    remove(&remote, &session.journal, &saved);
    let absent = key("never-present"); remove(&remote, &session.journal, &absent);
    assert!(store.video(&absent).unwrap().is_none());
    assert!(profile.0.join("audio/saved-job.wav").exists());
    let mut cursor = Cursor::default();
    assert!(once(&core, &mut cursor, &AtomicBool::new(false)).unwrap());
    for video in [&saved, &absent] { assert!(store.deleted(video).unwrap().unwrap().1); }
    assert!(store.cues(&saved).unwrap().is_empty());
    assert!(store.video_jobs(&saved).unwrap().is_empty());
    assert_eq!(store.library().unwrap()["playlists"][0]["name"], "Topic");
    assert_eq!(store.active_job().unwrap().unwrap().id, "active-job");
    for file in ["saved-job.wav", "saved-job.wav.sections"] { assert!(!profile.0.join("audio").join(file).exists()); }
    for file in ["active-job.wav", "unrelated.wav"] { assert!(profile.0.join("audio").join(file).exists()); }
    assert_eq!(std::fs::read(profile.0.join("models/shared.bin")).unwrap(), b"shared model");
    assert_eq!(core.store.library().unwrap(), original);
    let refusal = api::get_subtitles(&core, &saved).unwrap_err();
    assert_eq!(refusal.status, 410); assert!(refusal.deletion_token.is_some());
    session.journal.receive(remote.replica(), &remote.page(0).unwrap()).unwrap();
    once(&core, &mut cursor, &AtomicBool::new(false)).unwrap();
    assert!(store.video(&saved).unwrap().is_none());
}

#[test]
fn active_jobs_defer_cleanup_and_failed_files_do_not_starve_other_deletions_or_cross_sign_out() {
    let profile = Profile::new(); let remote_profile = Profile::new();
    let core = profile.core(); let session = core.account.session().unwrap();
    let store = session.journal.library().unwrap();
    let remote = Journal::open(&remote_profile.0, &VerifiedIdentity::fixture("cleanup-account")).unwrap();
    let active = key("active");
    store.create_job("active-job", &active, "https://lectures.example.org/active", "other", "Active lecture", 5000, None, "m").unwrap();
    std::fs::write(profile.0.join("audio/active-job.wav"), b"active audio").unwrap();
    remove(&remote, &session.journal, &active);
    let mut cursor = Cursor::default(); let stopped = AtomicBool::new(false);
    once(&core, &mut cursor, &stopped).unwrap();
    assert_eq!(store.active_job().unwrap().unwrap().id, "active-job");
    assert!(profile.0.join("audio/active-job.wav").exists());
    assert!(store.deleted(&active).unwrap().is_none());
    store.fail("active-job", "fixture_finished").unwrap();
    let blocked = profile.0.join("audio/active-job.blocked"); std::fs::create_dir(&blocked).unwrap();
    for i in 0..20 { remove(&remote, &session.journal, &key(&format!("absent-{i}"))); }
    let mut failures = 0;
    for _ in 0..4 {
        if let Err(error) = once(&core, &mut cursor, &stopped) { assert_eq!(error.error, "cleanup_failed"); failures += 1; }
    }
    assert!(failures > 0);
    assert!(!store.deleted(&active).unwrap().unwrap().1);
    for i in 0..20 { assert!(store.deleted(&key(&format!("absent-{i}"))).unwrap().unwrap().1); }
    let signed_out = core.account.sign_out(core.account.status().unwrap().generation).unwrap();
    std::fs::remove_dir(blocked).unwrap();
    assert!(!once(&core, &mut cursor, &stopped).unwrap());
    assert!(!store.deleted(&active).unwrap().unwrap().1);
    core.account.sign_in(signed_out.generation).unwrap();
    once(&core, &mut cursor, &stopped).unwrap();
    assert!(store.deleted(&active).unwrap().unwrap().1);
    assert!(store.video_jobs(&active).unwrap().is_empty());
}
