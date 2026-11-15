//! Native Google-account ownership. Page responses contain status, never stored credentials.

use crate::{
    google_auth::{AuthError, Platform, VerifiedIdentity, Verifier},
    google_desktop::Credentials,
    sync::Journal,
    sync_tls::DeviceKey,
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
};

pub trait NativeAccount: Send + Sync {
    fn sign_in(
        &self,
        nonce: &str,
        cancelled: &AtomicBool,
        verifier: &Verifier,
    ) -> Result<Credentials, AuthError>;
    fn read(&self) -> Result<Option<String>, AuthError>;
    fn write(&self, value: &str) -> Result<(), AuthError>;
    fn delete(&self) -> Result<(), AuthError>;
    fn platform(&self) -> Platform;
    fn cancel_sign_in(&self) {}
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    Unsupported,
    Loading,
    SignedOut,
    SigningIn,
    SignedIn,
    Expired,
    Unavailable,
}

#[derive(Clone, Serialize)]
pub struct Status {
    pub generation: u64,
    pub state: AuthState,
    pub email: Option<String>,
    pub error: Option<String>,
    pub sync_ready: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    id_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    nonce: String,
    #[serde(default)]
    device: Option<DeviceKey>,
    #[serde(default)]
    enrollment: Option<SavedEnrollment>,
}

pub(crate) const DEVICE_LIFETIME: i64 = 30 * 24 * 60 * 60;
pub(crate) fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

/// A native device grant, not a fresh Google token. It has no remote deserializer.
pub struct DeviceIdentity {
    subject: String,
    email: String,
    issued_at: i64,
    google_expires_at: i64,
    expires_at: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedEnrollment {
    subject: String,
    email: String,
    issued_at: i64,
    google_expires_at: i64,
    expires_at: i64,
    certificate: String,
}
impl DeviceIdentity {
    pub fn subject(&self) -> &str {
        &self.subject
    }
    pub fn email(&self) -> &str {
        &self.email
    }
    pub(crate) fn expires_at(&self) -> i64 {
        self.expires_at
    }
    fn require_current(&self) -> Result<(), AuthError> {
        if self.expires_at <= now() || self.issued_at > now() + 30 {
            Err(AuthError::Expired)
        } else {
            Ok(())
        }
    }
    fn from_verified(
        identity: &VerifiedIdentity,
        device: Option<&DeviceKey>,
    ) -> Result<Self, AuthError> {
        identity.require_current()?;
        let expires_at = match device {
            Some(device) => device.expires_at()?.min(
                identity
                    .issued_at()
                    .checked_add(DEVICE_LIFETIME)
                    .ok_or(AuthError::Invalid)?,
            ),
            None => identity.expires_at(),
        };
        let result = Self {
            subject: identity.subject().into(),
            email: identity.email().into(),
            issued_at: identity.issued_at(),
            google_expires_at: identity.expires_at(),
            expires_at,
        };
        result.require_current()?;
        Ok(result)
    }
    fn save(&self, certificate: String) -> SavedEnrollment {
        SavedEnrollment {
            subject: self.subject.clone(),
            email: self.email.clone(),
            issued_at: self.issued_at,
            google_expires_at: self.google_expires_at,
            expires_at: self.expires_at,
            certificate,
        }
    }
}
impl SavedEnrollment {
    // Only NativeAccount::read supplies this receipt, after OS-authenticated unsealing.
    fn restore(&self, device: &DeviceKey) -> Result<DeviceIdentity, AuthError> {
        if self.subject.is_empty()
            || self.subject.len() > 255
            || !self.subject.bytes().all(|b| b.is_ascii_graphic())
            || self.email.len() > 254
            || !self.email.contains('@')
            || self.email.chars().any(char::is_control)
            || self.issued_at < 0
            || self.issued_at > now() + 30
            || self.google_expires_at <= self.issued_at
            || self.expires_at <= self.issued_at
            || self.expires_at
                > self
                    .issued_at
                    .checked_add(DEVICE_LIFETIME)
                    .ok_or(AuthError::Invalid)?
            || self.expires_at > device.expires_at()?
            || self.certificate != device.nonce()
        {
            return Err(AuthError::Invalid);
        }
        let identity = DeviceIdentity {
            subject: self.subject.clone(),
            email: self.email.clone(),
            issued_at: self.issued_at,
            google_expires_at: self.google_expires_at,
            expires_at: self.expires_at,
        };
        Ok(identity)
    }
}

pub struct Session {
    pub identity: DeviceIdentity,
    pub journal: Journal,
    cancelled: Arc<AtomicBool>,
    device: Option<DeviceKey>,
    proof: String,
    platform: Platform,
}

pub struct LoginAttempt {
    pub generation: u64,
    cancelled: Arc<AtomicBool>,
    provider: Arc<dyn NativeAccount>,
}
pub(crate) struct LibraryScope {
    generation: u64,
    session: Option<Arc<Session>>,
}
impl LibraryScope {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub(crate) fn email(&self) -> Option<&str> {
        self.session.as_ref().map(|session| session.identity.email())
    }
}
impl Session {
    pub fn require_current(&self) -> Result<(), AuthError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(AuthError::Cancelled);
        }
        self.identity.require_current()
    }

    pub(crate) fn device(&self) -> Result<&DeviceKey, AuthError> {
        self.require_current()?;
        self.device.as_ref().ok_or(AuthError::Invalid)
    }

    pub(crate) fn peer_proof(&self) -> Result<(&str, Platform), AuthError> {
        self.device()?;
        if self.identity.google_expires_at <= now() {
            return Err(AuthError::Expired);
        }
        Ok((&self.proof, self.platform))
    }

    pub(crate) fn cancellation(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }
    pub(crate) fn platform(&self) -> Platform {
        self.platform
    }
}

struct State {
    generation: u64,
    restore_started: bool,
    auth: AuthState,
    error: Option<AuthError>,
    session: Option<Arc<Session>>,
    cancelled: Arc<AtomicBool>,
}
impl State {
    fn status(&self) -> Status {
        Status {
            generation: self.generation,
            state: self.auth,
            email: self.session.as_ref().map(|s| s.identity.email().into()),
            error: self.error.map(|e| e.to_string()),
            sync_ready: false,
        }
    }
    fn begin(&mut self, auth: AuthState) -> Result<(u64, Arc<AtomicBool>), AuthError> {
        self.cancelled.store(true, Ordering::Release);
        self.session = None;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(AuthError::Unavailable)?;
        self.cancelled = Arc::new(AtomicBool::new(false));
        self.auth = auth;
        self.error = None;
        Ok((self.generation, self.cancelled.clone()))
    }
    fn require(&self, generation: u64) -> Result<(), AuthError> {
        if self.generation != generation
            || (self.cancelled.load(Ordering::Acquire) && self.auth != AuthState::Expired)
        {
            Err(AuthError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Core owns one manager and verifier. Native app setup supplies one OS provider.
/// Login/restore callers own their worker; shutdown cancels its ticket before resources retire.
pub struct Manager {
    root: PathBuf,
    provider: Mutex<Option<Arc<dyn NativeAccount>>>,
    state: Mutex<State>,
    verifier: Verifier,
    stopped: AtomicBool,
    cancellation_flags: Mutex<Vec<Weak<AtomicBool>>>,
}

impl Manager {
    pub(crate) fn verifier(&self) -> &Verifier {
        &self.verifier
    }

    #[cfg(test)]
    pub(crate) fn with_test_verifier(root: PathBuf) -> Self {
        let mut manager = Self::new(root);
        manager.verifier = Verifier::with_test_keys(crate::google_auth::tests::keys());
        manager
    }
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            provider: Mutex::new(None),
            verifier: Verifier::default(),
            stopped: AtomicBool::new(false),
            cancellation_flags: Mutex::new(Vec::new()),
            state: Mutex::new(State {
                generation: 0,
                restore_started: false,
                auth: AuthState::Unsupported,
                error: None,
                session: None,
                cancelled: Arc::new(AtomicBool::new(false)),
            }),
        }
    }

    pub fn configure(&self, provider: Arc<dyn NativeAccount>) -> Result<(), AuthError> {
        self.require_running()?;
        let mut current = self.provider.lock().map_err(|_| AuthError::Unavailable)?;
        if current.is_some() {
            return Err(AuthError::Invalid);
        }
        let (_, cancelled) = self
            .state
            .lock()
            .map_err(|_| AuthError::Unavailable)?
            .begin(AuthState::Loading)?;
        self.track(&cancelled)?;
        *current = Some(provider);
        Ok(())
    }

    fn provider(&self) -> Result<Arc<dyn NativeAccount>, AuthError> {
        self.provider
            .lock()
            .map_err(|_| AuthError::Unavailable)?
            .clone()
            .ok_or(AuthError::Unavailable)
    }

    fn require_running(&self) -> Result<(), AuthError> {
        if self.stopped.load(Ordering::Acquire) {
            Err(AuthError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn track(&self, flag: &Arc<AtomicBool>) -> Result<(), AuthError> {
        let mut flags = self
            .cancellation_flags
            .lock()
            .map_err(|_| AuthError::Unavailable)?;
        if self.stopped.load(Ordering::Acquire) {
            flag.store(true, Ordering::Release);
            return Err(AuthError::Cancelled);
        }
        flags.retain(|flag| flag.strong_count() > 0);
        flags.push(Arc::downgrade(flag));
        Ok(())
    }

    pub fn status(&self) -> Result<Status, AuthError> {
        let mut state = self.state.lock().map_err(|_| AuthError::Unavailable)?;
        if state
            .session
            .as_ref()
            .is_some_and(|s| s.identity.require_current().is_err())
        {
            state.cancelled.store(true, Ordering::Release);
            state.auth = AuthState::Expired;
            state.error = Some(AuthError::Expired);
        }
        Ok(state.status())
    }

    pub fn restore(&self) -> Result<Status, AuthError> {
        self.require_running()?;
        let provider = self.provider()?;
        let (generation, cancelled) = {
            let mut state = self.state.lock().map_err(|_| AuthError::Unavailable)?;
            if state.auth != AuthState::Loading || state.restore_started {
                return Ok(state.status());
            }
            state.restore_started = true;
            (state.generation, state.cancelled.clone())
        };
        let result = (|| {
            let Some(raw) = provider.read()? else {
                return Ok(None);
            };
            if raw.len() > 64 * 1024 {
                return Err(AuthError::Invalid);
            }
            let saved: Saved = serde_json::from_str(&raw).map_err(|_| AuthError::Invalid)?;
            validate_device(&saved)?;
            let identity = if let Some(enrollment) = &saved.enrollment {
                enrollment.restore(saved.device.as_ref().ok_or(AuthError::Invalid)?)?
            } else {
                let verified =
                    self.verifier
                        .verify(&saved.id_token, &saved.nonce, provider.platform())?;
                DeviceIdentity::from_verified(&verified, None)?
            };
            if cancelled.load(Ordering::Acquire) {
                return Err(AuthError::Cancelled);
            }
            let journal =
                Journal::for_device(&self.root, &identity).map_err(|_| AuthError::Unavailable)?;
            Ok(Some(Arc::new(Session {
                identity,
                journal,
                cancelled: cancelled.clone(),
                device: saved.device,
                proof: saved.id_token,
                platform: provider.platform(),
            })))
        })();
        let mut state = self.state.lock().map_err(|_| AuthError::Unavailable)?;
        state.require(generation)?;
        match result {
            Ok(session) => {
                let expired = session
                    .as_ref()
                    .is_some_and(|s| s.identity.require_current().is_err());
                state.auth = if expired {
                    state.cancelled.store(true, Ordering::Release);
                    state.error = Some(AuthError::Expired);
                    AuthState::Expired
                } else if session.is_some() {
                    AuthState::SignedIn
                } else {
                    AuthState::SignedOut
                };
                state.session = session;
            }
            Err(error) => {
                state.auth = if error == AuthError::Expired {
                    AuthState::Expired
                } else {
                    AuthState::Unavailable
                };
                state.error = Some(error);
            }
        }
        Ok(state.status())
    }

    pub fn sign_in(&self, expected_generation: u64) -> Result<Status, AuthError> {
        self.finish_sign_in(self.begin_sign_in(expected_generation)?)
    }

    pub fn begin_sign_in(&self, expected_generation: u64) -> Result<LoginAttempt, AuthError> {
        self.require_running()?;
        let provider = self.provider()?;
        let (generation, cancelled) = {
            let mut state = self.state.lock().map_err(|_| AuthError::Unavailable)?;
            state.require(expected_generation)?;
            if state.auth == AuthState::SigningIn {
                return Err(AuthError::Invalid);
            }
            let ticket = state.begin(AuthState::SigningIn)?;
            if let Err(error) = provider.delete() {
                state.auth = AuthState::Unavailable;
                state.error = Some(error);
                return Err(error);
            }
            ticket
        };
        self.track(&cancelled)?;
        Ok(LoginAttempt {
            generation,
            cancelled,
            provider,
        })
    }

    pub fn pending_status(&self, attempt: &LoginAttempt) -> Result<Status, AuthError> {
        let state = self.state.lock().map_err(|_| AuthError::Unavailable)?;
        state.require(attempt.generation)?;
        Ok(state.status())
    }

    pub fn fail_attempt(&self, generation: u64) {
        if let Ok(mut state) = self.state.lock() {
            if state.require(generation).is_ok() {
                // The failed worker has retired; keep this generation usable for an explicit retry.
                state.auth = AuthState::Unavailable;
                state.error = Some(AuthError::Unavailable);
                state.session = None;
            }
        }
    }

    pub fn finish_sign_in(&self, attempt: LoginAttempt) -> Result<Status, AuthError> {
        let LoginAttempt {
            generation,
            cancelled,
            provider,
        } = attempt;
        let result = (|| {
            self.require_running()?;
            self.state
                .lock()
                .map_err(|_| AuthError::Unavailable)?
                .require(generation)?;
            let device = DeviceKey::generate()?;
            let nonce = device.nonce();
            let (_identity, id_token, refresh_token) = provider
                .sign_in(&nonce, &cancelled, &self.verifier)?
                .into_parts();
            if cancelled.load(Ordering::Acquire) {
                return Err(AuthError::Cancelled);
            }
            if id_token.len() > 8192
                || refresh_token
                    .as_ref()
                    .is_some_and(|value| value.len() > 16 * 1024)
            {
                return Err(AuthError::Invalid);
            }
            // The sealing boundary verifies the proof against this login's device key itself.
            let verified = self
                .verifier
                .verify(&id_token, &nonce, provider.platform())?;
            let identity = DeviceIdentity::from_verified(&verified, Some(&device))?;
            let saved = Saved {
                id_token,
                refresh_token: None,
                nonce: nonce.clone(),
                device: Some(device),
                enrollment: Some(identity.save(nonce.clone())),
            };
            let serialized = serde_json::to_string(&saved).map_err(|_| AuthError::Invalid)?;
            let mut state = self.state.lock().map_err(|_| AuthError::Unavailable)?;
            state.require(generation)?;
            self.require_running()?;
            let journal =
                Journal::for_device(&self.root, &identity).map_err(|_| AuthError::Unavailable)?;
            provider.write(&serialized)?;
            state.session = Some(Arc::new(Session {
                identity,
                journal,
                cancelled: cancelled.clone(),
                device: saved.device,
                proof: saved.id_token,
                platform: provider.platform(),
            }));
            state.auth = AuthState::SignedIn;
            Ok(state.status())
        })();
        if let Err(error) = result {
            let mut state = self.state.lock().map_err(|_| AuthError::Unavailable)?;
            state.require(generation)?;
            state.auth = AuthState::SignedOut;
            state.error = Some(error);
        }
        result
    }

    pub fn sign_out(&self, expected_generation: u64) -> Result<Status, AuthError> {
        self.require_running()?;
        let provider = self.provider()?;
        let result = {
            let mut state = self.state.lock().map_err(|_| AuthError::Unavailable)?;
            state.require(expected_generation)?;
            state.begin(AuthState::SignedOut)?;
            provider.cancel_sign_in();
            match provider.delete() {
                Ok(()) => Ok(state.status()),
                Err(error) => {
                    state.auth = AuthState::Unavailable;
                    state.error = Some(error);
                    Err(error)
                }
            }
        };
        result
    }

    pub fn session(&self) -> Result<Arc<Session>, AuthError> {
        let state = self.state.lock().map_err(|_| AuthError::Unavailable)?;
        let session = state.session.clone().ok_or(AuthError::Invalid)?;
        session.require_current()?;
        Ok(session)
    }

    pub(crate) fn capture_library(&self) -> Result<LibraryScope, AuthError> {
        self.require_running()?;
        let state = self.state.lock().map_err(|_| AuthError::Unavailable)?;
        if !matches!(
            state.auth,
            AuthState::Unsupported
                | AuthState::SignedOut
                | AuthState::SignedIn
                | AuthState::Expired
        ) {
            return Err(AuthError::Unavailable);
        }
        Ok(LibraryScope {
            generation: state.generation,
            session: state.session.clone(),
        })
    }

    pub(crate) fn with_library<T>(
        &self,
        scope: &LibraryScope,
        f: impl FnOnce(Option<&Journal>) -> Result<T, String>,
    ) -> Result<T, AuthError> {
        self.require_running()?;
        let state = self.state.lock().map_err(|_| AuthError::Unavailable)?;
        let same = match (&scope.session, &state.session) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if scope.generation != state.generation
            || !same
            || !matches!(
                state.auth,
                AuthState::Unsupported
                    | AuthState::SignedOut
                    | AuthState::SignedIn
                    | AuthState::Expired
            )
        {
            return Err(AuthError::Cancelled);
        }
        f(scope.session.as_ref().map(|s| &s.journal)).map_err(|_| AuthError::Unavailable)
    }

    pub fn with_session<T>(
        &self,
        session: &Arc<Session>,
        f: impl FnOnce(&Journal) -> Result<T, String>,
    ) -> Result<T, AuthError> {
        self.require_running()?;
        let state = self.state.lock().map_err(|_| AuthError::Unavailable)?;
        session.require_current()?;
        if !state
            .session
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, session))
        {
            return Err(AuthError::Cancelled);
        }
        f(&session.journal).map_err(|_| AuthError::Unavailable)
    }

    pub(crate) fn peer_trust(
        &self,
        session: &Arc<Session>,
        certificate: &crate::sync_tls::PeerCertificate,
    ) -> Result<Option<i64>, AuthError> {
        session.device()?;
        let expiry = self.with_session(session, |journal| {
            journal.peer_expiry(certificate.fingerprint())
        })?;
        Ok(expiry
            .map(|e| {
                e.min(session.identity.expires_at())
                    .min(certificate.expires_at())
            })
            .filter(|e| *e > now()))
    }

    pub(crate) fn enroll_peer(
        &self,
        session: &Arc<Session>,
        peer: &crate::sync_tls::PeerIdentity,
    ) -> Result<i64, AuthError> {
        session.device()?;
        peer.require_fresh()?;
        if peer.subject() != session.identity.subject() {
            return Err(AuthError::AccountMismatch);
        }
        let expiry = peer.expires_at().min(session.identity.expires_at());
        if expiry <= now() {
            return Err(AuthError::Expired);
        }
        self.with_session(session, |journal| {
            journal.trust_peer(peer.certificate(), expiry)
        })?;
        Ok(expiry)
    }

    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::Release);
        if let Ok(flags) = self.cancellation_flags.lock() {
            for flag in flags.iter().filter_map(Weak::upgrade) {
                flag.store(true, Ordering::Release);
            }
        }
        // Exit runs on the UI thread; never wait for an operation awaiting a native UI callback.
        if let Ok(mut state) = self.state.try_lock() {
            let _ = state.begin(AuthState::SignedOut);
            state.cancelled.store(true, Ordering::Release);
        }
    }
}

fn validate_device(saved: &Saved) -> Result<(), AuthError> {
    if let Some(device) = &saved.device {
        if device.nonce() != saved.nonce {
            return Err(AuthError::Invalid);
        }
        device.client_config()?;
        device.server_config()?;
        device.expires_at()?;
    }
    Ok(())
}

#[cfg(windows)]
pub struct WindowsAccount(crate::account_secrets::SecretFile);
#[cfg(windows)]
impl WindowsAccount {
    pub fn new(root: &std::path::Path) -> Self {
        Self(crate::account_secrets::SecretFile::for_folder(root))
    }
}
#[cfg(windows)]
impl NativeAccount for WindowsAccount {
    fn platform(&self) -> Platform {
        Platform::Desktop
    }
    fn read(&self) -> Result<Option<String>, AuthError> {
        self.0.get()
    }
    fn write(&self, value: &str) -> Result<(), AuthError> {
        self.0.set(value)
    }
    fn delete(&self) -> Result<(), AuthError> {
        self.0.delete()
    }
    fn sign_in(
        &self,
        nonce: &str,
        cancelled: &AtomicBool,
        verifier: &Verifier,
    ) -> Result<Credentials, AuthError> {
        let login = crate::google_desktop::DesktopLogin::bind(nonce)?;
        std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", login.authorization_url()])
            .spawn()
            .map_err(|_| AuthError::Unavailable)?;
        login.complete(cancelled, verifier)
    }
}

#[cfg(test)]
#[path = "account_tests.rs"]
pub(crate) mod tests;
