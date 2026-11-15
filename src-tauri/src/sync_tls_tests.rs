use super::*;
use openidconnect::{
    core::{
        CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
        CoreRsaPrivateSigningKey,
    },
    JsonWebKeyId, PrivateSigningKey,
};
use rustls::{ClientConnection, Connection, ServerConnection};
use std::io::{Cursor, Write};

fn pair(client: Arc<ClientConfig>, server: Arc<ServerConfig>) -> (Connection, Connection) {
    (
        Connection::Client(ClientConnection::new(client, server_name()).unwrap()),
        Connection::Server(ServerConnection::new(server).unwrap()),
    )
}
fn exchange(from: &mut Connection, to: &mut Connection) -> Result<(), Error> {
    let mut bytes = Vec::new();
    while from.wants_write() {
        from.write_tls(&mut bytes).unwrap();
    }
    if !bytes.is_empty() {
        to.read_tls(&mut Cursor::new(bytes)).unwrap();
        to.process_new_packets()?;
    }
    Ok(())
}
fn handshake(client: &mut Connection, server: &mut Connection) -> Result<(), Error> {
    for _ in 0..16 {
        exchange(client, server)?;
        exchange(server, client)?;
        if !client.is_handshaking() && !server.is_handshaking() {
            return Ok(());
        }
    }
    panic!("fixture handshake did not finish");
}
fn proof(subject: &str, nonce: &str, expired: bool) -> (Verifier, String) {
    let key = CoreRsaPrivateSigningKey::from_pem(
        include_str!("../tests/fixtures/auth/test-only-rsa.pem"),
        Some(JsonWebKeyId::new("test".into())),
    )
    .unwrap();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let claims: CoreIdTokenClaims = serde_json::from_value(serde_json::json!({"iss":"https://accounts.google.com","sub":subject,"email":"fixture@example.org","email_verified":true,"aud":crate::google_auth::ANDROID_AUDIENCE,"iat":now-3600,"exp":if expired {now-1} else {now+3600},"nonce":nonce})).unwrap();
    let token = CoreIdToken::new(
        claims,
        &key,
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        None,
        None,
    )
    .unwrap()
    .to_string();
    (
        Verifier::with_test_keys(CoreJsonWebKeySet::new(vec![key.as_verification_key()])),
        token,
    )
}

#[test]
fn mutual_tls_checks_both_device_keys_before_a_matching_google_identity_can_authorize() {
    let phone = DeviceKey::generate().unwrap();
    let pc = DeviceKey::generate().unwrap();
    let (mut client, mut server) =
        pair(phone.client_config().unwrap(), pc.server_config().unwrap());
    assert!(PeerCertificate::from_tls(&client).is_err());
    handshake(&mut client, &mut server).unwrap();
    let remote = PeerCertificate::from_tls(&server).unwrap();
    assert_eq!(remote.fingerprint, phone.nonce());
    assert_eq!(
        PeerCertificate::from_tls(&client).unwrap().fingerprint,
        pc.nonce()
    );
    let (verifier, token) = proof("same-account", &phone.nonce(), false);
    let local = VerifiedIdentity::fixture("same-account");
    assert_eq!(
        remote
            .verify_google(&verifier, &local, &token, Platform::Android)
            .unwrap()
            .subject(),
        "same-account"
    );
    assert!(matches!(
        remote.verify_google(
            &verifier,
            &VerifiedIdentity::fixture("other-account"),
            &token,
            Platform::Android
        ),
        Err(AuthError::AccountMismatch)
    ));
    assert!(matches!(
        remote.verify_google(&verifier, &local, &token, Platform::Desktop),
        Err(AuthError::Invalid)
    ));
    let (verifier, expired) = proof("same-account", &phone.nonce(), true);
    assert!(matches!(
        remote.verify_google(&verifier, &local, &expired, Platform::Android),
        Err(AuthError::Expired)
    ));
}

#[test]
fn a_copied_google_proof_does_not_authorize_a_different_tls_key_and_ciphertext_cannot_replay() {
    let original = DeviceKey::generate().unwrap();
    let attacker = DeviceKey::generate().unwrap();
    let pc = DeviceKey::generate().unwrap();
    let (verifier, stolen) = proof("same-account", &original.nonce(), false);
    let (mut client, mut server) = pair(
        attacker.client_config().unwrap(),
        pc.server_config().unwrap(),
    );
    handshake(&mut client, &mut server).unwrap();
    assert!(matches!(
        PeerCertificate::from_tls(&server).unwrap().verify_google(
            &verifier,
            &VerifiedIdentity::fixture("same-account"),
            &stolen,
            Platform::Android
        ),
        Err(AuthError::Invalid)
    ));
    client
        .writer()
        .write_all(b"old encrypted application record")
        .unwrap();
    let mut captured = Vec::new();
    client.write_tls(&mut captured).unwrap();
    let (mut fresh_client, mut fresh_server) = pair(
        attacker.client_config().unwrap(),
        pc.server_config().unwrap(),
    );
    handshake(&mut fresh_client, &mut fresh_server).unwrap();
    fresh_server.read_tls(&mut Cursor::new(captured)).unwrap();
    assert!(fresh_server.process_new_packets().is_err());
}

#[test]
fn missing_client_keys_wrong_alpn_malformed_certificates_and_mismatched_private_keys_fail() {
    let phone = DeviceKey::generate().unwrap();
    let pc = DeviceKey::generate().unwrap();
    let mut http = (*phone.client_config().unwrap()).clone();
    http.alpn_protocols = vec![b"http/1.1".to_vec()];
    let (mut client, mut server) = pair(Arc::new(http), pc.server_config().unwrap());
    assert!(handshake(&mut client, &mut server).is_err());
    let mut no_key =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_protocol_versions(&[&rustls::version::TLS13])
            .unwrap()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(KeyPossession))
            .with_no_client_auth();
    no_key.alpn_protocols = vec![ALPN.to_vec()];
    let (mut client, mut server) = pair(Arc::new(no_key), pc.server_config().unwrap());
    assert!(handshake(&mut client, &mut server).is_err());
    assert!(roots(&CertificateDer::from(vec![0u8; MAX_CERT + 1]), &[]).is_err());
    assert!(roots(&CertificateDer::from(vec![0u8; 16]), &[]).is_err());
    let mut mismatched = DeviceKey::generate().unwrap();
    mismatched.certificate = phone.certificate.clone();
    assert!(mismatched.client_config().is_err());
    assert!(mismatched.server_config().is_err());
}

#[test]
fn expired_device_certificates_fail_and_sealed_key_roundtrips_keep_the_google_nonce() {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::new(vec![NAME.into()]).unwrap();
    params.not_before = time::OffsetDateTime::now_utc() - time::Duration::days(2);
    params.not_after = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    let expired = DeviceKey {
        certificate: params.self_signed(&key).unwrap().der().to_vec(),
        private_key: key.serialize_der(),
    };
    let current = DeviceKey::generate().unwrap();
    let (mut client, mut server) = pair(
        current.client_config().unwrap(),
        expired.server_config().unwrap(),
    );
    assert!(handshake(&mut client, &mut server).is_err());
    let saved = serde_json::to_vec(&current).unwrap();
    let restored: DeviceKey = serde_json::from_slice(&saved).unwrap();
    assert_eq!(restored.nonce(), current.nonce());
    let (mut client, mut server) = pair(
        restored.client_config().unwrap(),
        current.server_config().unwrap(),
    );
    handshake(&mut client, &mut server).unwrap();
}
