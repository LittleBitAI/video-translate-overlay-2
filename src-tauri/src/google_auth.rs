//! Google identity verification for native sessions. No credentials are exposed through IPC.

use openidconnect::{
    core::{CoreIdToken, CoreIdTokenVerifier, CoreJsonWebKeySet, CoreJwsSigningAlgorithm},
    ClaimsVerificationError, ClientId, IssuerUrl, Nonce, SignatureVerificationError,
};
use std::{
    str::FromStr,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub use crate::google_registration::DESKTOP_CLIENT_ID;
pub const ANDROID_CLIENT_ID: &str =
    "1072382283165-1fifmmjgf3k0ff15h1fe7aie4jdb1h5i.apps.googleusercontent.com";
pub const ANDROID_AUDIENCE: &str =
    "1072382283165-3e4i7184unnrqunbr212i42hbg8fpics.apps.googleusercontent.com";
const ISSUER: &str = "https://accounts.google.com";
const JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const MAX_TOKEN_BYTES: usize = 8192;
const MAX_JWKS_BYTES: usize = 256 * 1024;
const KEY_LIFETIME: Duration = Duration::from_secs(300);
const FETCH_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Desktop,
    Android,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthError {
    Invalid,
    Expired,
    Unavailable,
    AccountMismatch,
    Cancelled,
    Denied,
    TimedOut,
    ProviderRejected,
    Registration,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Invalid => "invalid Google authentication",
            Self::Expired => "Google authentication expired",
            Self::Unavailable => "Google identity verification unavailable",
            Self::AccountMismatch => "Google accounts do not match",
            Self::Cancelled => "Google sign-in cancelled",
            Self::Denied => "Google sign-in was declined",
            Self::TimedOut => "Google sign-in timed out",
            Self::ProviderRejected => "Google sign-in failed",
            Self::Registration => "Google Desktop registration is missing or invalid",
        })
    }
}
impl std::error::Error for AuthError {}

/// Construction requires a signed Google token. Neither email nor a supplied subject authorizes a session.
#[derive(Clone)]
pub struct VerifiedIdentity {
    subject: String,
    email: String,
    expires_at: i64,
}

impl VerifiedIdentity {
    pub fn subject(&self) -> &str {
        &self.subject
    }
    pub fn email(&self) -> &str {
        &self.email
    }
    pub fn expires_at(&self) -> i64 {
        self.expires_at
    }

    pub fn require_current(&self) -> Result<(), AuthError> {
        if self.expires_at <= now() {
            Err(AuthError::Expired)
        } else {
            Ok(())
        }
    }

    pub fn require_same_account(&self, peer: &Self) -> Result<(), AuthError> {
        self.require_current()?;
        peer.require_current()?;
        if self.subject == peer.subject {
            Ok(())
        } else {
            Err(AuthError::AccountMismatch)
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture(subject: &str) -> Self {
        Self {
            subject: subject.into(),
            email: "offline-fixture@example.org".into(),
            expires_at: now() + 3600,
        }
    }

    #[cfg(test)]
    pub(crate) fn expired_fixture(subject: &str) -> Self {
        let mut identity = Self::fixture(subject);
        identity.expires_at = now() - 1;
        identity
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[derive(Default)]
struct Cache {
    keys: Option<(Instant, CoreJsonWebKeySet)>,
    attempted_at: Option<Instant>,
}

/// The native core owns and shares one verifier. Only public signing keys are cached.
/// Fetches are coalesced under the mutex, time out after five seconds, and leave no open response.
#[derive(Default)]
pub struct Verifier {
    cache: Mutex<Cache>,
}

impl Verifier {
    pub fn verify(
        &self,
        token: &str,
        expected_nonce: &str,
        platform: Platform,
    ) -> Result<VerifiedIdentity, AuthError> {
        self.verify_using(token, expected_nonce, platform, fetch_keys)
    }

    fn verify_using(
        &self,
        raw: &str,
        expected_nonce: &str,
        platform: Platform,
        fetch: impl FnOnce() -> Result<CoreJsonWebKeySet, AuthError>,
    ) -> Result<VerifiedIdentity, AuthError> {
        if raw.len() > MAX_TOKEN_BYTES
            || !(32..=128).contains(&expected_nonce.len())
            || !expected_nonce
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(AuthError::Invalid);
        }
        let token = CoreIdToken::from_str(raw).map_err(|_| AuthError::Invalid)?;
        let nonce = Nonce::new(expected_nonce.to_string());
        let mut cache = self.cache.lock().map_err(|_| AuthError::Unavailable)?;
        let fresh = cache
            .keys
            .as_ref()
            .filter(|(at, _)| at.elapsed() < KEY_LIFETIME);
        if let Some((_, keys)) = fresh {
            match verify_token(&token, &nonce, platform, keys) {
                Ok(identity) => return Ok(identity),
                Err(ClaimsVerificationError::SignatureVerification(
                    SignatureVerificationError::NoMatchingKey,
                )) => {}
                Err(e) => return Err(public_error(e)),
            }
        }
        if cache
            .attempted_at
            .is_some_and(|at| at.elapsed() < FETCH_COOLDOWN)
        {
            return Err(if fresh.is_some() {
                AuthError::Invalid
            } else {
                AuthError::Unavailable
            });
        }
        cache.attempted_at = Some(Instant::now());
        let keys = fetch()?;
        let result = verify_token(&token, &nonce, platform, &keys).map_err(public_error);
        cache.keys = Some((Instant::now(), keys));
        result
    }
}

fn public_error(error: ClaimsVerificationError) -> AuthError {
    match error {
        ClaimsVerificationError::Expired(_) => AuthError::Expired,
        _ => AuthError::Invalid,
    }
}

fn verify_token(
    token: &CoreIdToken,
    nonce: &Nonce,
    platform: Platform,
    keys: &CoreJsonWebKeySet,
) -> Result<VerifiedIdentity, ClaimsVerificationError> {
    let audience = match platform {
        Platform::Desktop => DESKTOP_CLIENT_ID,
        Platform::Android => ANDROID_AUDIENCE,
    };
    let verifier = CoreIdTokenVerifier::new_public_client(
        ClientId::new(audience.into()),
        IssuerUrl::new(ISSUER.into()).expect("constant Google issuer"),
        keys.clone(),
    )
    .set_allowed_algs([CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256]);
    let claims = token.claims(&verifier, nonce)?;
    let subject = claims.subject().as_str();
    let email = claims
        .email()
        .map(|value| value.as_str())
        .unwrap_or_default();
    let party_valid = claims
        .authorized_party()
        .is_none_or(|party| match platform {
            Platform::Desktop => party.as_str() == DESKTOP_CLIENT_ID,
            Platform::Android => {
                party.as_str() == ANDROID_CLIENT_ID || party.as_str() == ANDROID_AUDIENCE
            }
        });
    if claims.audiences().len() != 1
        || !party_valid
        || claims.email_verified() != Some(true)
        || subject.is_empty()
        || subject.len() > 255
        || !subject.bytes().all(|b| b.is_ascii_graphic())
        || email.len() > 254
        || !email.contains('@')
        || email.chars().any(char::is_control)
        || claims.issue_time().timestamp() > now() + 30
        || claims.issue_time().timestamp() < 0
        || claims.expiration() <= claims.issue_time()
    {
        return Err(ClaimsVerificationError::Other(
            "invalid identity claims".into(),
        ));
    }
    Ok(VerifiedIdentity {
        subject: subject.into(),
        email: email.into(),
        expires_at: claims.expiration().timestamp(),
    })
}

fn fetch_keys() -> Result<CoreJsonWebKeySet, AuthError> {
    let mut request = crate::http::Request::get(JWKS_URL);
    request.timeout_ms = 5000;
    request.follow_redirects = false;
    let response = crate::http::send(request).map_err(|_| AuthError::Unavailable)?;
    if response.status != 200 {
        return Err(AuthError::Unavailable);
    }
    let bytes = response
        .bytes(MAX_JWKS_BYTES + 1)
        .map_err(|_| AuthError::Unavailable)?;
    if bytes.len() > MAX_JWKS_BYTES {
        return Err(AuthError::Unavailable);
    }
    let keys: CoreJsonWebKeySet =
        serde_json::from_slice(&bytes).map_err(|_| AuthError::Unavailable)?;
    if keys.keys().is_empty() || keys.keys().len() > 16 {
        return Err(AuthError::Unavailable);
    }
    Ok(keys)
}

#[cfg(test)]
#[path = "google_auth_tests.rs"]
mod tests;
