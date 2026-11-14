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
        _nonce: &str,
        _cancelled: &AtomicBool,
        _verifier: &Verifier,
    ) -> Result<Credentials, AuthError> {
        let subject = self.subject.lock().unwrap().clone();
        let gate = self.gate.lock().unwrap().take();
        if let Some((started, release)) = gate {
            started.send(()).unwrap();
            release.recv_timeout(Duration::from_secs(30)).unwrap();
        }
        Ok(Credentials {
            identity: VerifiedIdentity::fixture(&subject),
            id_token: format!("offline-token-{subject}"),
            refresh_token: Some("offline-refresh-token".into()),
        })
    }
}
fn configured(profile: &Profile) -> (Arc<Manager>, Arc<Native>) {
    let manager = Arc::new(Manager::new(profile.0.clone()));
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
    ] {
        assert!(!serialized.contains(field), "status contains {field}");
    }
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
            identity: VerifiedIdentity::expired_fixture("account-one"),
            journal: Journal::open(&profile.0, &VerifiedIdentity::fixture("account-one")).unwrap(),
            cancelled: state.cancelled.clone(),
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
