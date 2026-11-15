use super::*;
use openidconnect::{
    core::{CoreIdTokenClaims, CoreRsaPrivateSigningKey},
    JsonWebKeyId, PrivateSigningKey,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

const NONCE: &str = "9d53c2d19d3b82ed94901d58a24dab623191dad252a7eb36730dcfb75915b515";
const PRIVATE_KEY: &str = include_str!("../tests/fixtures/auth/test-only-rsa.pem");

fn key(id: &str) -> CoreRsaPrivateSigningKey {
    CoreRsaPrivateSigningKey::from_pem(PRIVATE_KEY, Some(JsonWebKeyId::new(id.into()))).unwrap()
}
pub(crate) fn keys() -> CoreJsonWebKeySet {
    CoreJsonWebKeySet::new(vec![key("test").as_verification_key()])
}
pub(crate) fn claims(platform: Platform) -> Value {
    json!({"iss":ISSUER,"sub":"verified-google-subject-one",
        "aud":if platform==Platform::Desktop { DESKTOP_CLIENT_ID } else { ANDROID_AUDIENCE },
        "exp":now()+3600,"iat":now(),"nonce":NONCE,"email":"one@example.org","email_verified":true})
}
pub(crate) fn signed(value: Value) -> String {
    sign_with(value, "test", CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256)
}
fn sign_with(value: Value, kid: &str, algorithm: CoreJwsSigningAlgorithm) -> String {
    let claims: CoreIdTokenClaims = serde_json::from_value(value).unwrap();
    CoreIdToken::new(claims, &key(kid), algorithm, None, None)
        .unwrap()
        .to_string()
}
fn verify(value: Value, platform: Platform) -> Result<VerifiedIdentity, AuthError> {
    Verifier::default().verify_using(&signed(value), NONCE, platform, || Ok(keys()))
}

#[test]
fn signed_identities_use_the_registered_audience_and_stable_subject() {
    let desktop = verify(claims(Platform::Desktop), Platform::Desktop).unwrap();
    assert_eq!(desktop.subject(), "verified-google-subject-one");
    assert_eq!(desktop.email(), "one@example.org");
    assert!(desktop.expires_at() > now());
    let mut value = claims(Platform::Android);
    value["azp"] = ANDROID_CLIENT_ID.into();
    value["email"] = "renamed@example.org".into();
    let android = verify(value.clone(), Platform::Android).unwrap();
    desktop.require_same_account(&android).unwrap();
    value["sub"] = "verified-google-subject-two".into();
    value["email"] = "one@example.org".into();
    let different = verify(value, Platform::Android).unwrap();
    assert_eq!(
        desktop.require_same_account(&different),
        Err(AuthError::AccountMismatch)
    );
}

#[test]
fn wrong_issuer_client_party_nonce_and_unverified_or_invalid_claims_are_rejected() {
    for (field, value) in [
        ("iss", json!("https://other.example.org")),
        ("aud", json!("another-client.apps.googleusercontent.com")),
        ("aud", json!([DESKTOP_CLIENT_ID, "unrelated-client"])),
        ("azp", json!(ANDROID_CLIENT_ID)),
        (
            "nonce",
            json!("another-connection-nonce-000000000000000000000000000"),
        ),
        ("email_verified", json!(false)),
        ("sub", json!("")),
        ("sub", json!("subject\nwith-control")),
        ("email", json!("no-email-address")),
        ("email", json!("one@example.org\n")),
        ("iat", json!(now() + 300)),
        ("iat", json!(-1)),
    ] {
        let mut input = claims(Platform::Desktop);
        input[field] = value;
        let result = verify(input, Platform::Desktop);
        assert!(matches!(result, Err(AuthError::Invalid)), "field {field}");
    }
    let mut input = claims(Platform::Android);
    input["azp"] = DESKTOP_CLIENT_ID.into();
    assert!(matches!(
        verify(input, Platform::Android),
        Err(AuthError::Invalid)
    ));
    for field in ["nonce", "email_verified", "email"] {
        let mut input = claims(Platform::Desktop);
        input.as_object_mut().unwrap().remove(field);
        assert!(
            matches!(verify(input, Platform::Desktop), Err(AuthError::Invalid)),
            "missing {field}"
        );
    }
}

#[test]
fn expired_or_tampered_tokens_and_disallowed_signature_algorithms_fail_closed() {
    let mut expired = claims(Platform::Desktop);
    expired["iat"] = json!(now() - 3600);
    expired["exp"] = json!(now() - 1);
    assert!(matches!(
        verify(expired, Platform::Desktop),
        Err(AuthError::Expired)
    ));
    let verifier = Verifier::default();
    let valid = signed(claims(Platform::Desktop));
    verifier
        .verify_using(&valid, NONCE, Platform::Desktop, || Ok(keys()))
        .unwrap();
    let mut tampered = valid.into_bytes();
    let signature = tampered.iter().rposition(|b| *b == b'.').unwrap() + 1;
    tampered[signature] = if tampered[signature] == b'A' {
        b'B'
    } else {
        b'A'
    };
    assert!(matches!(
        verifier.verify_using(
            std::str::from_utf8(&tampered).unwrap(),
            NONCE,
            Platform::Desktop,
            || panic!("must use cached keys")
        ),
        Err(AuthError::Invalid)
    ));
    let other_alg = sign_with(
        claims(Platform::Desktop),
        "test",
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha512,
    );
    assert!(matches!(
        verifier.verify_using(&other_alg, NONCE, Platform::Desktop, || panic!(
            "must use cached keys"
        )),
        Err(AuthError::Invalid)
    ));
}

#[test]
fn concurrent_verification_coalesces_public_key_fetches_and_failed_fetches_back_off() {
    let verifier = Arc::new(Verifier::default());
    let fetches = Arc::new(AtomicUsize::new(0));
    let token = signed(claims(Platform::Desktop));
    let start = Arc::new(std::sync::Barrier::new(5));
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let (verifier, fetches, token, start) = (
                verifier.clone(),
                fetches.clone(),
                token.clone(),
                start.clone(),
            );
            std::thread::spawn(move || {
                start.wait();
                verifier
                    .verify_using(&token, NONCE, Platform::Desktop, || {
                        fetches.fetch_add(1, Ordering::SeqCst);
                        Ok(keys())
                    })
                    .unwrap()
                    .subject()
                    .to_string()
            })
        })
        .collect();
    start.wait();
    for thread in threads {
        assert_eq!(thread.join().unwrap(), "verified-google-subject-one");
    }
    assert_eq!(fetches.load(Ordering::SeqCst), 1);

    let unavailable = Verifier::default();
    assert!(matches!(
        unavailable.verify_using(&token, NONCE, Platform::Desktop, || Err(
            AuthError::Unavailable
        )),
        Err(AuthError::Unavailable)
    ));
    assert!(matches!(
        unavailable.verify_using(&token, NONCE, Platform::Desktop, || panic!(
            "failed fetch must back off"
        )),
        Err(AuthError::Unavailable)
    ));
}

#[test]
fn rotated_signing_keys_refetch_after_cooldown_without_accepting_an_unknown_key() {
    let verifier = Verifier::default();
    let initial = signed(claims(Platform::Desktop));
    verifier
        .verify_using(&initial, NONCE, Platform::Desktop, || Ok(keys()))
        .unwrap();
    let rotated = sign_with(
        claims(Platform::Desktop),
        "rotated",
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
    );
    assert!(matches!(
        verifier.verify_using(&rotated, NONCE, Platform::Desktop, || panic!(
            "unknown key must respect cooldown"
        )),
        Err(AuthError::Invalid)
    ));
    verifier.cache.lock().unwrap().attempted_at = Some(Instant::now() - FETCH_COOLDOWN);
    let rotated_keys = CoreJsonWebKeySet::new(vec![key("rotated").as_verification_key()]);
    verifier
        .verify_using(&rotated, NONCE, Platform::Desktop, || Ok(rotated_keys))
        .unwrap();
}

#[test]
fn malformed_or_oversized_authentication_never_fetches_keys_or_leaks_the_input() {
    let verifier = Verifier::default();
    for input in [
        "private-input-do-not-log".into(),
        "x".repeat(MAX_TOKEN_BYTES + 1),
    ] {
        let result = verifier.verify_using(&input, NONCE, Platform::Desktop, || {
            panic!("malformed token must not fetch")
        });
        assert!(matches!(result, Err(AuthError::Invalid)));
        assert_eq!(
            result.err().unwrap().to_string(),
            "invalid Google authentication"
        );
    }
    assert!(matches!(
        verifier.verify_using(
            &signed(claims(Platform::Desktop)),
            "short",
            Platform::Desktop,
            || panic!("invalid nonce must not fetch")
        ),
        Err(AuthError::Invalid)
    ));
    let mut identity = VerifiedIdentity::fixture("one");
    identity.expires_at = now() - 1;
    assert_eq!(identity.require_current(), Err(AuthError::Expired));
    assert_eq!(
        identity.require_same_account(&VerifiedIdentity::fixture("one")),
        Err(AuthError::Expired)
    );
}
