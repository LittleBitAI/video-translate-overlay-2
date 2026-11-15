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
