use super::*;
use crate::sync::{Change, Deletions};
use std::{path::PathBuf, sync::mpsc, time::Duration};

struct Profile(PathBuf);
impl Profile {
    fn new() -> Self {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let id: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("vto-account-{id}"));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[derive(Default)]
struct Native {
    subject: Mutex<String>,
    saved: Mutex<Option<String>>,
    gate: Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>>,
    fail_write: AtomicBool,
    wrong_nonce: AtomicBool,
}
impl NativeAccount for Native {
    fn platform(&self) -> Platform {
        Platform::Desktop
    }
    fn read(&self) -> Result<Option<String>, AuthError> {
        Ok(self.saved.lock().unwrap().clone())
    }
    fn write(&self, value: &str) -> Result<(), AuthError> {
        if self.fail_write.load(Ordering::Acquire) {
            return Err(AuthError::Unavailable);
        }
        *self.saved.lock().unwrap() = Some(value.into());
        Ok(())
    }
    fn delete(&self) -> Result<(), AuthError> {
        *self.saved.lock().unwrap() = None;
        Ok(())
    }
    fn sign_in(
        &self,
        nonce: &str,
        _cancelled: &AtomicBool,
        verifier: &Verifier,
    ) -> Result<Credentials, AuthError> {
        let subject = self.subject.lock().unwrap().clone();
        let gate = self.gate.lock().unwrap().take();
        if let Some((started, release)) = gate {
            started.send(()).unwrap();
            release.recv_timeout(Duration::from_secs(30)).unwrap();
        }
        let actual_nonce = if self.wrong_nonce.load(Ordering::Acquire) {
            "wrong-device-nonce-000000000000000000000000000000000000000000000"
        } else {
            nonce
        };
        let mut claims = crate::google_auth::tests::claims(Platform::Desktop);
        claims["sub"] = subject.into();
        claims["nonce"] = actual_nonce.into();
        let token = crate::google_auth::tests::signed(claims);
        Ok(Credentials {
            identity: verifier.verify(&token, actual_nonce, Platform::Desktop)?,
            id_token: token,
            refresh_token: Some("offline-refresh-token".into()),
        })
    }
}
fn configured(profile: &Profile) -> (Arc<Manager>, Arc<Native>) {
    let manager = Arc::new(Manager::with_test_verifier(profile.0.clone()));
    let native = Arc::new(Native::default());
    *native.subject.lock().unwrap() = "account-one".into();
    manager.configure(native.clone()).unwrap();
    let pending = manager.status().unwrap();
    assert_eq!(pending.state, AuthState::Loading);
    let restored = manager.restore().unwrap();
    assert_eq!(restored.state, AuthState::SignedOut);
    assert_eq!(restored.generation, pending.generation);
    assert_eq!(manager.restore().unwrap().generation, restored.generation);
    (manager, native)
}

pub(crate) fn signed_in_manager(root: &std::path::Path, subject: &str) -> Manager {
    let manager = Manager::with_test_verifier(root.into());
    let native = Arc::new(Native::default());
    *native.subject.lock().unwrap() = subject.into();
    manager.configure(native).unwrap();
    manager.restore().unwrap();
    manager.sign_in(manager.status().unwrap().generation).unwrap();
    manager
}

#[test]
fn production_library_requests_select_the_account_and_copy_legacy_data_only_with_current_consent() {
    use crate::api::{library_request, video_key};
    use serde_json::json;
    let profile = Profile::new();
    let mut core = crate::pipeline::worker_tests::idle_core(&profile.0, "");
    core.account = Manager::with_test_verifier(profile.0.clone());
    let native = Arc::new(Native::default());
    *native.subject.lock().unwrap() = "library-one".into();
    core.account.configure(native.clone()).unwrap();
    core.account.restore().unwrap();
    let first = core.account.sign_in(core.account.status().unwrap().generation).unwrap().generation;
    let url = "https://lectures.example.org/saved";
    let key = video_key(&url::Url::parse(url).unwrap());
    core.store.create_job("legacy", &key, url, "other", "Existing lecture", 5000, None, "original-model").unwrap();
    core.store.save_region("legacy", &key, 0, &[(123, 2345, "original".into())]).unwrap();
    core.store.set_translations("legacy", &[(0, "translated".into())]).unwrap();
    core.store.set_terms(&key, "[]").unwrap();
    core.store.add_usage("legacy", 12, 4, |_, _| 12.5).unwrap();
    core.store.finish("legacy").unwrap();
    core.store.create_playlist("legacy-list", "Existing playlist").unwrap();
    core.store.add_entry("legacy-list", &key, url, "other", "Existing lecture").unwrap();
    let original = core.store.library().unwrap();
    let read = |generation| library_request(&core, Some(generation), "get_library", &json!({})).unwrap();
    assert_eq!(read(first)["videos"], json!([]));
    assert_eq!(read(first)["legacy_available"], true);
    assert_eq!(library_request(&core, None, "get_library", &json!({})).unwrap_err().error, "account_required");
    let no_consent = json!({"action":{"action":"import_legacy"}});
    assert_eq!(library_request(&core, Some(first), "library_action", &no_consent).unwrap_err().error, "confirmation_required");
    assert_eq!(core.store.library().unwrap(), original);
    let consent = json!({"action":{"action":"import_legacy","confirmed":true}});
    library_request(&core, Some(first), "library_action", &consent).unwrap();
    assert_eq!(read(first)["videos"][0]["saved"], true);
    assert_eq!(read(first)["legacy_available"], false);
    let subtitle = library_request(&core, Some(first), "get_subtitles", &json!({"video_key":key})).unwrap();
    assert_eq!(subtitle["cues"][0]["start_ms"], 123);
    assert_eq!(subtitle["cues"][0]["end_ms"], 2345);
    let job = library_request(&core, Some(first), "get_job", &json!({"page_url":url})).unwrap();
    assert_eq!(job["cost_krw"], 12.5);
    let old = core.library(Some(first)).unwrap();
    *native.subject.lock().unwrap() = "library-two".into();
    let second = core.account.sign_in(first).unwrap().generation;
    assert_eq!(read(second)["videos"], json!([]));
    assert_eq!(read(second)["playlists"], json!([]));
    assert_eq!(old.run(|store| Ok(store.library()?)).unwrap_err().error, "account_changed");
    assert_eq!(library_request(&core, Some(first), "library_action", &consent).unwrap_err().error, "account_changed");
    let stale_delete = json!({"action":{"action":"delete_video","video_key":key,"title":"Existing lecture","confirmed":true}});
    assert_eq!(library_request(&core, Some(first), "library_action", &stale_delete).unwrap_err().error, "account_changed");
    let signed_out = core.account.sign_out(second).unwrap().generation;
    assert_eq!(read(signed_out)["videos"], original["videos"]);
    assert_eq!(core.store.library().unwrap(), original);
    *native.subject.lock().unwrap() = "library-one".into();
    let returned = core.account.sign_in(signed_out).unwrap().generation;
    assert_eq!(read(returned)["videos"][0]["saved"], true);
    assert_eq!(read(returned)["playlists"][0]["name"], "Existing playlist");
    assert_eq!(read(returned)["legacy_available"], false);
    core.store.create_playlist("later", "New signed-out playlist").unwrap();
    assert_eq!(read(returned)["legacy_available"], true);
    library_request(&core, Some(returned), "library_action", &consent).unwrap();
    assert_eq!(read(returned)["legacy_available"], false);
    assert_eq!(read(returned)["playlists"].as_array().unwrap().len(), 2);
    crate::db::Db::open(core.store.path()).unwrap().execute("UPDATE cue SET text_ko=NULL", &[]).unwrap();
    assert_eq!(read(returned)["legacy_available"], true);
    assert_eq!(library_request(&core, Some(returned), "library_action", &consent).unwrap_err().status, 503);
    assert_eq!(read(returned)["videos"][0]["saved"], true);
}

#[test]
fn playback_requests_are_account_scoped_durable_and_leave_completed_subtitles_unchanged() {
    use crate::api::{library_request, video_key};
    use serde_json::json;
    let profile = Profile::new();
    let mut core = crate::pipeline::worker_tests::idle_core(&profile.0, "");
    core.account = signed_in_manager(&profile.0, "playback-one");
    let generation = core.account.status().unwrap().generation;
    let session = core.account.session().unwrap();
    let store = session.journal.library().unwrap();
    let url = "https://lectures.example.org/playback";
    let key = video_key(&url::Url::parse(url).unwrap());
    store.create_job("saved", &key, url, "other", "Saved", 60000, None, "original-model").unwrap();
    store.save_region("saved", &key, 0, &[(123, 2345, "original".into())]).unwrap();
    store.set_translations("saved", &[(0, "translated".into())]).unwrap();
    store.set_terms(&key, "[]").unwrap();
    store.add_usage("saved", 12, 4, |_, _| 12.5).unwrap();
    store.finish("saved").unwrap();
    let original = store.library().unwrap()["videos"][0]["versions"][0]["id"].as_str().unwrap().to_string();
    let mut alternate = session.journal.records().unwrap().into_iter().find_map(|record| match record.change {
        Change::Translation { translation } => Some(translation), _ => None,
    }).unwrap();
    alternate.cues[0].text_ko = "Other retained translation".into();
    alternate.cost_krw = 18.5;
    session.journal.write(Change::Translation { translation: alternate }, Deletions::new()).unwrap();
    let selection = json!({"action":{"action":"select_translation","video_key":key,"translation_id":original}});
    library_request(&core, Some(generation), "library_action", &selection).unwrap();
    let set = |position, duration| library_request(&core, Some(generation), "get_job", &json!({"page_url":url,"playback_action":"set","position_ms":position,"duration_ms":duration}));
    assert_eq!(set(12345, 60000).unwrap()["position_ms"], 12345);
    let through = session.journal.page(0).unwrap().through;
    set(12345, 60000).unwrap();
    assert_eq!(session.journal.page(0).unwrap().through, through);
    assert_eq!(set(-1, 60000).unwrap_err().status, 400);
    assert_eq!(set(60001, 60000).unwrap_err().status, 400);
    assert_eq!(set(100, 3000).unwrap_err().status, 409);
    assert_eq!(library_request(&core, Some(generation), "get_job", &json!({"page_url":url,"playback_action":"set","position_ms":1.5,"duration_ms":60000})).unwrap_err().status, 400);
    let other = Profile::new();
    let identity = crate::google_auth::VerifiedIdentity::fixture("playback-one");
    let received = crate::sync::Journal::open(&other.0, &identity).unwrap();
    received.receive(session.journal.replica(), &session.journal.page(0).unwrap()).unwrap();
    let reopened = received.library().unwrap();
    assert_eq!(reopened.playback(&key).unwrap(), Some(12345));
    assert_eq!(reopened.cues(&key).unwrap()[0].start_ms, 123);
    assert_eq!(reopened.cues(&key).unwrap()[0].end_ms, 2345);
    assert_eq!(reopened.ready_job(&key).unwrap().unwrap().cost_krw, 12.5);
    core.account.sign_out(generation).unwrap();
    assert_eq!(library_request(&core, Some(generation), "library_action", &selection).unwrap_err().status, 409);
    assert_eq!(set(30000, 60000).unwrap_err().status, 409);
    assert_eq!(store.playback(&key).unwrap(), Some(12345));
    assert_eq!(core.store.playback(&key).unwrap(), None);
    core.store.create_playlist("root", "Root").unwrap();
    core.store.add_entry("root", &key, url, "other", "Root video").unwrap();
    core.store.save_playback(&key, 333, 60000).unwrap();
    core.store.begin_delete(&key, "root-deletion-token").unwrap();
    core.store.finish_delete(&key, None).unwrap();
    assert_eq!(core.store.playback(&key).unwrap(), None);
}

#[test]
fn switching_accounts_while_a_media_check_is_pending_does_not_create_a_job_in_either_account() {
    use crate::{api, AudioExtractor};
    struct Gate {
        started: mpsc::SyncSender<()>,
        release: Mutex<mpsc::Receiver<()>>,
        url: String,
    }
    impl AudioExtractor for Gate {
        fn audio_url(&self, _: &str) -> Result<String, crate::media::Refusal> {
            self.started.send(()).unwrap();
            self.release.lock().unwrap().recv_timeout(Duration::from_secs(10)).unwrap();
            Ok(self.url.clone())
        }
    }
    let profile = Profile::new();
    let mut core = crate::pipeline::worker_tests::idle_core(&profile.0, "");
    core.account = Manager::with_test_verifier(profile.0.clone());
    let native = Arc::new(Native::default());
    *native.subject.lock().unwrap() = "pending-one".into();
    core.account.configure(native.clone()).unwrap();
    core.account.restore().unwrap();
    let first = core.account.sign_in(core.account.status().unwrap().generation).unwrap().generation;
    let old = core.account.session().unwrap();
    let (started, observe) = mpsc::sync_channel(1);
    let (release, wait) = mpsc::channel();
    core.svc.extractor = Box::new(Gate { started, release: Mutex::new(wait), url: crate::api::duration_tests::serve_wav() });
    let core = Arc::new(core);
    let creating = core.clone();
    let request = std::thread::spawn(move || api::create_job(&creating, "https://youtu.be/abcDEF12345", None, "Pending lecture", 5000));
    observe.recv_timeout(Duration::from_secs(10)).unwrap();
    *native.subject.lock().unwrap() = "pending-two".into();
    core.account.sign_in(first).unwrap();
    release.send(()).unwrap();
    assert_eq!(request.join().unwrap().unwrap_err().error, "account_changed");
    assert!(!core.has_active_job().unwrap());
    assert_eq!(old.journal.library().unwrap().library().unwrap()["videos"], serde_json::json!([]));
    assert_eq!(api::get_library(&core).unwrap()["videos"], serde_json::json!([]));
    assert!(core.store.library().unwrap()["videos"].as_array().unwrap().is_empty());
}

#[test]
fn unsupported_hosts_fail_closed_and_status_never_contains_credentials() {
    let profile = Profile::new();
    let manager = Manager::new(profile.0.clone());
    assert_eq!(manager.status().unwrap().state, AuthState::Unsupported);
    assert!(matches!(manager.sign_in(0), Err(AuthError::Unavailable)));
    let (manager, _) = configured(&profile);
    let result = manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    assert_eq!(result.state, AuthState::SignedIn);
    assert!(
        !result.sync_ready,
        "identity setup alone cannot claim working synchronization"
    );
    let serialized = serde_json::to_string(&result).unwrap();
    for field in [
        "offline-token",
        "offline-refresh-token",
        "nonce",
        "id_token",
        "refresh_token",
        "private_key",
        "certificate",
    ] {
        assert!(!serialized.contains(field), "status contains {field}");
    }
}

#[test]
fn each_login_seals_a_new_device_key_bound_to_its_google_nonce() {
    let profile = Profile::new();
    let (manager, native) = configured(&profile);
    manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    let old = manager.session().unwrap();
    let first_nonce = old.device().unwrap().nonce();
    let saved: Saved =
        serde_json::from_str(native.saved.lock().unwrap().as_ref().unwrap()).unwrap();
    assert_eq!(saved.nonce, first_nonce);
    validate_device(&saved).unwrap();
    assert!(saved.enrollment.as_ref().unwrap().expires_at > now() + DEVICE_LIFETIME - 60);
    assert!(saved.refresh_token.is_none());
    let mut malformed = serde_json::to_value(&saved).unwrap();
    malformed["nonce"] = "another-device-certificate-000000000000000000000000000000000".into();
    assert_eq!(
        validate_device(&serde_json::from_value(malformed).unwrap()),
        Err(AuthError::Invalid)
    );
    let mut malformed = serde_json::to_value(&saved).unwrap();
    malformed["device"]["private_key"] = serde_json::json!([0, 1, 2]);
    assert_eq!(
        validate_device(&serde_json::from_value(malformed).unwrap()),
        Err(AuthError::Invalid)
    );
    manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    assert!(old.peer_proof().is_err());
    assert_ne!(
        manager.session().unwrap().device().unwrap().nonce(),
        first_nonce
    );
    let session = manager.session().unwrap();
    assert_eq!(session.peer_proof().unwrap().1, Platform::Desktop);
}

#[test]
fn sealing_rejects_a_providers_valid_google_proof_for_another_device_nonce() {
    let profile = Profile::new();
    let (manager, native) = configured(&profile);
    native.wrong_nonce.store(true, Ordering::Release);
    assert!(matches!(
        manager.sign_in(manager.status().unwrap().generation),
        Err(AuthError::Invalid)
    ));
    assert!(native.saved.lock().unwrap().is_none());
    assert!(manager.session().is_err());
}

#[test]
fn sealed_device_enrollment_restores_without_google_and_expiry_cannot_be_extended() {
    let profile = Profile::new();
    let (manager, native) = configured(&profile);
    manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    let original = manager.session().unwrap();
    let certificate = original.device().unwrap().nonce();
    drop(original);
    manager.shutdown();
    drop(manager);
    // An ordinary manager has no cached fixture keys: a successful restore must use the native receipt.
    let restored = Manager::new(profile.0.clone());
    restored.configure(native.clone()).unwrap();
    assert_eq!(restored.restore().unwrap().state, AuthState::SignedIn);
    assert_eq!(
        restored.session().unwrap().device().unwrap().nonce(),
        certificate
    );
    restored.shutdown();
    drop(restored);
    let raw = native.saved.lock().unwrap().clone().unwrap();
    for invalid in ["extended", "wrong_certificate", "expired"] {
        let mut saved: Saved = serde_json::from_str(&raw).unwrap();
        let receipt = saved.enrollment.as_mut().unwrap();
        match invalid {
            "extended" => receipt.expires_at += DEVICE_LIFETIME,
            "wrong_certificate" => receipt.certificate = "0".repeat(64),
            "expired" => {
                receipt.issued_at = now() - DEVICE_LIFETIME - 86400;
                receipt.google_expires_at = receipt.issued_at + 3600;
                receipt.expires_at = receipt.issued_at + DEVICE_LIFETIME;
            }
            _ => unreachable!(),
        }
        *native.saved.lock().unwrap() = Some(serde_json::to_string(&saved).unwrap());
        let attempt = Manager::new(profile.0.clone());
        attempt.configure(native.clone()).unwrap();
        let status = attempt.restore().unwrap();
        assert_ne!(status.state, AuthState::SignedIn, "{invalid}");
        assert!(attempt.session().is_err(), "{invalid}");
    }
}

pub(crate) fn historical_receipt(subject: &str, platform: Platform, created_at: i64) -> String {
    let device = DeviceKey::historical_for_test(created_at);
    let nonce = device.nonce();
    let mut claims = crate::google_auth::tests::claims(platform);
    claims["sub"] = subject.into();
    claims["nonce"] = nonce.clone().into();
    claims["iat"] = (created_at + 60).into();
    claims["exp"] = (created_at + 3660).into();
    let token = crate::google_auth::tests::signed(claims);
    let saved = Saved {
        id_token: token,
        refresh_token: None,
        nonce: nonce.clone(),
        enrollment: Some(SavedEnrollment {
            subject: subject.into(),
            email: "one@example.org".into(),
            issued_at: created_at + 60,
            google_expires_at: created_at + 3660,
            expires_at: device.expires_at().unwrap(),
            certificate: nonce.clone(),
        }),
        device: Some(device),
    };
    serde_json::to_string(&saved).unwrap()
}

#[test]
fn a_historical_native_receipt_restores_after_its_google_proof_expires_without_extending_that_proof(
) {
    let profile = Profile::new();
    let created_at = now() - 7200;
    let raw = historical_receipt("account-one", Platform::Desktop, created_at);
    let saved: Saved = serde_json::from_str(&raw).unwrap();
    let nonce = saved.nonce.clone();
    assert!(matches!(
        Verifier::with_test_keys(crate::google_auth::tests::keys()).verify(
            &saved.id_token,
            &nonce,
            Platform::Desktop
        ),
        Err(AuthError::Expired)
    ));
    let native = Arc::new(Native::default());
    *native.saved.lock().unwrap() = Some(raw);
    let manager = Manager::new(profile.0.clone());
    manager.configure(native).unwrap();
    assert_eq!(manager.restore().unwrap().state, AuthState::SignedIn);
    let session = manager.session().unwrap();
    assert!(session.require_current().is_ok());
    assert!(matches!(session.peer_proof(), Err(AuthError::Expired)));
    assert_eq!(session.device().unwrap().nonce(), nonce);
    assert_eq!(session.identity.expires_at(), created_at + DEVICE_LIFETIME);
}

#[test]
fn credential_expiry_keeps_local_library_ownership_and_account_switching_retires_that_scope() {
    let profile = Profile::new();
    let native = Arc::new(Native::default());
    *native.subject.lock().unwrap() = "account-two".into();
    *native.saved.lock().unwrap() = Some(historical_receipt(
        "account-one",
        Platform::Desktop,
        now() - DEVICE_LIFETIME - 86400,
    ));
    let manager = Manager::with_test_verifier(profile.0.clone());
    manager.configure(native.clone()).unwrap();
    assert_eq!(manager.restore().unwrap().state, AuthState::Expired);
    assert!(manager.session().is_err());
    let local = manager.capture_library().unwrap();
    manager
        .with_library(&local, |journal| {
            journal
                .unwrap()
                .library()?
                .create_playlist("offline", "Kept locally")
        })
        .unwrap();
    assert_eq!(local.generation(), manager.status().unwrap().generation);
    manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    assert!(matches!(
        manager.with_library::<()>(&local, |_| panic!("retired account scope must not write")),
        Err(AuthError::Cancelled)
    ));
    let newer = manager.capture_library().unwrap();
    manager
        .with_library(&newer, |journal| {
            assert!(journal.unwrap().library()?.library()?["playlists"]
                .as_array()
                .unwrap()
                .is_empty());
            Ok(())
        })
        .unwrap();
    *native.subject.lock().unwrap() = "account-one".into();
    manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    let original = manager.capture_library().unwrap();
    manager
        .with_library(&original, |journal| {
            assert_eq!(
                journal.unwrap().library()?.library()?["playlists"][0]["id"],
                "offline"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn account_changes_cancel_old_tickets_and_keep_unsent_data_in_its_original_account() {
    let profile = Profile::new();
    let (manager, native) = configured(&profile);
    let first = manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    let old = manager.session().unwrap();
    manager
        .with_session(&old, |journal| {
            journal
                .write(
                    Change::PlaylistName {
                        id: "topic".into(),
                        name: "Unsent topic".into(),
                    },
                    Deletions::new(),
                )
                .map(drop)
        })
        .unwrap();
    manager.sign_out(first.generation).unwrap();
    assert_eq!(old.require_current(), Err(AuthError::Cancelled));
    assert!(matches!(
        manager.with_session::<()>(&old, |_| panic!("retired session must not write")),
        Err(AuthError::Cancelled)
    ));
    *native.subject.lock().unwrap() = "account-two".into();
    let second = manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    let current = manager.session().unwrap();
    assert_ne!(old.journal.account(), current.journal.account());
    assert!(current.journal.records().unwrap().is_empty());
    manager.sign_out(second.generation).unwrap();
    *native.subject.lock().unwrap() = "account-one".into();
    manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    assert_eq!(
        manager.session().unwrap().journal.records().unwrap().len(),
        1
    );
}

#[test]
fn a_late_sign_in_cannot_replace_the_new_account_or_rewrite_its_saved_credentials() {
    let profile = Profile::new();
    let (manager, native) = configured(&profile);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    *native.gate.lock().unwrap() = Some((started_tx, release_rx));
    let generation = manager.status().unwrap().generation;
    let pending = {
        let manager = manager.clone();
        std::thread::spawn(move || manager.sign_in(generation))
    };
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    manager
        .sign_out(manager.status().unwrap().generation)
        .unwrap();
    *native.subject.lock().unwrap() = "account-two".into();
    manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    let saved = native.saved.lock().unwrap().clone();
    release_tx.send(()).unwrap();
    assert!(matches!(pending.join().unwrap(), Err(AuthError::Cancelled)));
    assert_eq!(*native.saved.lock().unwrap(), saved);
    assert_eq!(manager.session().unwrap().identity.subject(), "account-two");
}

#[test]
fn stale_screens_and_failed_credential_writes_cannot_create_a_signed_in_session() {
    let profile = Profile::new();
    let (manager, native) = configured(&profile);
    assert!(matches!(manager.sign_in(0), Err(AuthError::Cancelled)));
    native.fail_write.store(true, Ordering::Release);
    assert!(matches!(
        manager.sign_in(manager.status().unwrap().generation),
        Err(AuthError::Unavailable)
    ));
    assert_eq!(manager.status().unwrap().state, AuthState::SignedOut);
    assert!(manager.session().is_err());
    assert!(native.saved.lock().unwrap().is_none());
}

#[test]
fn pending_receipts_allow_cancellation_before_native_login_and_failed_workers_can_be_retried() {
    let profile = Profile::new();
    let (manager, native) = configured(&profile);
    let attempt = manager
        .begin_sign_in(manager.status().unwrap().generation)
        .unwrap();
    let receipt = manager.pending_status(&attempt).unwrap();
    assert_eq!(receipt.state, AuthState::SigningIn);
    let (started_tx, started_rx) = mpsc::channel();
    let (_release_tx, release_rx) = mpsc::channel();
    *native.gate.lock().unwrap() = Some((started_tx, release_rx));
    manager.sign_out(receipt.generation).unwrap();
    assert!(manager.pending_status(&attempt).is_err());
    assert!(matches!(
        manager.finish_sign_in(attempt),
        Err(AuthError::Cancelled)
    ));
    assert!(matches!(
        started_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    native.gate.lock().unwrap().take();

    let attempt = manager
        .begin_sign_in(manager.status().unwrap().generation)
        .unwrap();
    manager.fail_attempt(attempt.generation);
    assert_eq!(manager.status().unwrap().state, AuthState::Unavailable);
    let result = manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    assert_eq!(result.state, AuthState::SignedIn);
    manager.fail_attempt(attempt.generation);
    assert_eq!(manager.status().unwrap().state, AuthState::SignedIn);
}

#[test]
fn expiration_and_shutdown_revoke_existing_session_tickets() {
    let profile = Profile::new();
    let (manager, _) = configured(&profile);
    manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    let original = manager.session().unwrap();
    {
        let mut state = manager.state.lock().unwrap();
        state.session = Some(Arc::new(Session {
            identity: DeviceIdentity {
                subject: "account-one".into(),
                email: "fixture@example.org".into(),
                issued_at: now() - DEVICE_LIFETIME,
                google_expires_at: now() - 1,
                expires_at: now() - 1,
            },
            journal: Journal::open(&profile.0, &VerifiedIdentity::fixture("account-one")).unwrap(),
            cancelled: state.cancelled.clone(),
            device: None,
            proof: String::new(),
            platform: Platform::Desktop,
        }));
    }
    assert_eq!(manager.status().unwrap().state, AuthState::Expired);
    assert_eq!(original.require_current(), Err(AuthError::Cancelled));
    assert!(manager.session().is_err());
    manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    let current = manager.session().unwrap();
    manager.shutdown();
    assert_eq!(current.require_current(), Err(AuthError::Cancelled));
    assert_eq!(manager.status().unwrap().state, AuthState::SignedOut);
    assert!(matches!(
        manager.sign_in(manager.status().unwrap().generation),
        Err(AuthError::Cancelled)
    ));
}

#[test]
fn shutdown_cancels_sessions_without_waiting_for_a_native_operation_holding_state() {
    let profile = Profile::new();
    let (manager, _) = configured(&profile);
    manager
        .sign_in(manager.status().unwrap().generation)
        .unwrap();
    let session = manager.session().unwrap();
    let _native_operation = manager.state.lock().unwrap();
    manager.shutdown();
    assert_eq!(session.require_current(), Err(AuthError::Cancelled));
    assert!(matches!(
        manager.with_session::<()>(&session, |_| panic!("shutdown must not write")),
        Err(AuthError::Cancelled)
    ));
}
