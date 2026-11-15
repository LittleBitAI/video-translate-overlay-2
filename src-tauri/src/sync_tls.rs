//! Device-key possession is checked by TLS; Google authorization is a separate gate.
use crate::google_auth::{AuthError, Platform, VerifiedIdentity, Verifier};
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
    server::danger::{ClientCertVerified, ClientCertVerifier},
    ClientConfig, CommonState, DigitallySignedStruct, DistinguishedName, Error, RootCertStore,
    ServerConfig, SignatureScheme,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use zeroize::Zeroize;

const NAME: &str = "vto-sync.local";
pub(crate) const ALPN: &[u8] = b"vto-sync/1";
const MAX_CERT: usize = 4096;

/// One key per native login, sealed alongside its Google proof; never a library record.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeviceKey {
    certificate: Vec<u8>,
    private_key: Vec<u8>,
}
impl Drop for DeviceKey {
    fn drop(&mut self) {
        self.private_key.zeroize();
    }
}
impl DeviceKey {
    pub fn generate() -> Result<Self, AuthError> {
        Self::generate_at(time::OffsetDateTime::now_utc())
    }
    #[cfg(test)]
    pub(crate) fn historical_for_test(created_at: i64) -> Self {
        Self::generate_at(time::OffsetDateTime::from_unix_timestamp(created_at).unwrap()).unwrap()
    }
    fn generate_at(created_at: time::OffsetDateTime) -> Result<Self, AuthError> {
        let key = rcgen::KeyPair::generate().map_err(|_| AuthError::Unavailable)?;
        let mut params =
            rcgen::CertificateParams::new(vec![NAME.into()]).map_err(|_| AuthError::Unavailable)?;
        params.distinguished_name = rcgen::DistinguishedName::new();
        params.not_before = created_at - time::Duration::minutes(5);
        params.not_after = created_at + time::Duration::seconds(crate::account::DEVICE_LIFETIME);
        params.extended_key_usages = vec![
            rcgen::ExtendedKeyUsagePurpose::ServerAuth,
            rcgen::ExtendedKeyUsagePurpose::ClientAuth,
        ];
        let certificate = params
            .self_signed(&key)
            .map_err(|_| AuthError::Unavailable)?
            .der()
            .to_vec();
        Ok(Self {
            certificate,
            private_key: key.serialize_der(),
        })
    }

    pub fn nonce(&self) -> String {
        fingerprint(&self.certificate)
    }

    pub fn expires_at(&self) -> Result<i64, AuthError> {
        certificate_expiry(&self.certificate)
    }

    fn material(
        &self,
    ) -> Result<
        (
            Vec<CertificateDer<'static>>,
            rustls::pki_types::PrivateKeyDer<'static>,
        ),
        AuthError,
    > {
        if self.certificate.is_empty()
            || self.certificate.len() > MAX_CERT
            || self.private_key.is_empty()
            || self.private_key.len() > MAX_CERT
        {
            return Err(AuthError::Invalid);
        }
        Ok((
            vec![CertificateDer::from(self.certificate.clone())],
            PrivatePkcs8KeyDer::from(self.private_key.clone()).into(),
        ))
    }

    pub fn client_config(&self) -> Result<Arc<ClientConfig>, AuthError> {
        let (certs, key) = self.material()?;
        let mut config =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_protocol_versions(&[&rustls::version::TLS13])
                .map_err(|_| AuthError::Unavailable)?
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(KeyPossession))
                .with_client_auth_cert(certs, key)
                .map_err(|_| AuthError::Invalid)?;
        config.alpn_protocols = vec![ALPN.to_vec()];
        config.resumption = rustls::client::Resumption::disabled();
        config.enable_early_data = false;
        Ok(Arc::new(config))
    }

    pub fn server_config(&self) -> Result<Arc<ServerConfig>, AuthError> {
        let (certs, key) = self.material()?;
        let mut config =
            ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_protocol_versions(&[&rustls::version::TLS13])
                .map_err(|_| AuthError::Unavailable)?
                .with_client_cert_verifier(Arc::new(KeyPossession))
                .with_single_cert(certs, key)
                .map_err(|_| AuthError::Invalid)?;
        config.alpn_protocols = vec![ALPN.to_vec()];
        config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
        config.send_tls13_tickets = 0;
        config.max_early_data_size = 0;
        Ok(Arc::new(config))
    }
}

pub(crate) fn server_name() -> ServerName<'static> {
    ServerName::try_from(NAME.to_string()).expect("constant DNS name")
}

fn fingerprint(certificate: &[u8]) -> String {
    Sha256::digest(certificate)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// TLS has checked this peer's possession of the key, not its Google account.
/// Construction requires a finished, dedicated TLS 1.3 handshake with a client certificate.
pub(crate) struct PeerCertificate {
    fingerprint: String,
    expires_at: i64,
}
pub(crate) struct PeerIdentity {
    identity: VerifiedIdentity,
    certificate: String,
    expires_at: i64,
}
impl PeerIdentity {
    pub fn subject(&self) -> &str {
        self.identity.subject()
    }
    pub fn certificate(&self) -> &str {
        &self.certificate
    }
    pub fn expires_at(&self) -> i64 {
        self.expires_at
    }
    pub fn require_fresh(&self) -> Result<(), AuthError> {
        self.identity.require_current()
    }
}
fn certificate_expiry(certificate: &[u8]) -> Result<i64, AuthError> {
    if certificate.is_empty() || certificate.len() > MAX_CERT {
        return Err(AuthError::Invalid);
    }
    let (rest, parsed) =
        x509_parser::parse_x509_certificate(certificate).map_err(|_| AuthError::Invalid)?;
    if !rest.is_empty() {
        return Err(AuthError::Invalid);
    }
    Ok(parsed.validity().not_after.timestamp())
}
impl PeerCertificate {
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    pub fn expires_at(&self) -> i64 {
        self.expires_at
    }
    pub fn from_tls(tls: &CommonState) -> Result<Self, AuthError> {
        if tls.is_handshaking()
            || tls.protocol_version() != Some(rustls::ProtocolVersion::TLSv1_3)
            || tls.alpn_protocol() != Some(ALPN)
        {
            return Err(AuthError::Invalid);
        }
        let certs = tls.peer_certificates().ok_or(AuthError::Invalid)?;
        if certs.len() != 1 || certs[0].is_empty() || certs[0].len() > MAX_CERT {
            return Err(AuthError::Invalid);
        }
        Ok(Self {
            fingerprint: fingerprint(certs[0].as_ref()),
            expires_at: certificate_expiry(certs[0].as_ref())?,
        })
    }

    pub fn verify_google(
        &self,
        verifier: &Verifier,
        token: &str,
        platform: Platform,
    ) -> Result<PeerIdentity, AuthError> {
        let peer = verifier.verify(token, &self.fingerprint, platform)?;
        let expires_at = self.expires_at.min(
            peer.issued_at()
                .checked_add(crate::account::DEVICE_LIFETIME)
                .ok_or(AuthError::Invalid)?,
        );
        Ok(PeerIdentity {
            identity: peer,
            certificate: self.fingerprint.clone(),
            expires_at,
        })
    }
}

/// Quarantine accepts only a bounded, valid certificate and a real TLS signature.
/// No library operation is authorized until PeerCertificate::verify_google succeeds.
#[derive(Debug)]
struct KeyPossession;
fn roots(
    cert: &CertificateDer<'_>,
    chain: &[CertificateDer<'_>],
) -> Result<Arc<RootCertStore>, Error> {
    if cert.is_empty() || cert.len() > MAX_CERT || !chain.is_empty() {
        return Err(Error::InvalidCertificate(
            rustls::CertificateError::BadEncoding,
        ));
    }
    let mut roots = RootCertStore::empty();
    roots.add(cert.clone().into_owned())?;
    Ok(Arc::new(roots))
}
fn signature(
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
) -> Result<HandshakeSignatureValid, Error> {
    rustls::crypto::verify_tls13_signature(
        message,
        cert,
        dss,
        &rustls::crypto::ring::default_provider().signature_verification_algorithms,
    )
}
fn schemes() -> Vec<SignatureScheme> {
    rustls::crypto::ring::default_provider()
        .signature_verification_algorithms
        .supported_schemes()
}
fn no_tls12() -> Error {
    Error::General("sync requires TLS 1.3".into())
}

impl ServerCertVerifier for KeyPossession {
    fn verify_server_cert(
        &self,
        cert: &CertificateDer<'_>,
        chain: &[CertificateDer<'_>],
        name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        rustls::client::WebPkiServerVerifier::builder_with_provider(
            roots(cert, chain)?,
            Arc::new(rustls::crypto::ring::default_provider()),
        )
        .build()
        .map_err(|_| Error::General("invalid sync certificate".into()))?
        .verify_server_cert(cert, chain, name, ocsp, now)
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Err(no_tls12())
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        signature(message, cert, dss)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        schemes()
    }
}
impl ClientCertVerifier for KeyPossession {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }
    fn verify_client_cert(
        &self,
        cert: &CertificateDer<'_>,
        chain: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        rustls::server::WebPkiClientVerifier::builder_with_provider(
            roots(cert, chain)?,
            Arc::new(rustls::crypto::ring::default_provider()),
        )
        .build()
        .map_err(|_| Error::General("invalid sync certificate".into()))?
        .verify_client_cert(cert, chain, now)
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Err(no_tls12())
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        signature(message, cert, dss)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        schemes()
    }
}

#[cfg(test)]
#[path = "sync_tls_tests.rs"]
mod tests;
