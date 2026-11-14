//! One Windows OAuth attempt: arm a loopback callback before opening the system browser.

use crate::google_auth::{AuthError, Platform, VerifiedIdentity, Verifier, DESKTOP_CLIENT_ID};
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreJsonWebKeySet},
    AuthType, AuthUrl, AuthorizationCode, ClientId, CsrfToken, EndpointNotSet, EndpointSet,
    HttpRequest, HttpResponse, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_HEADER_BYTES: usize = 8192;
const MAX_TOKEN_RESPONSE: usize = 32 * 1024;
type Client = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
    EndpointNotSet,
>;

/// Native-only result. It deliberately implements neither Debug nor Serialize.
pub struct Credentials {
    pub identity: VerifiedIdentity,
    pub(crate) id_token: String,
    pub(crate) refresh_token: Option<String>,
}

impl Credentials {
    pub fn into_parts(self) -> (VerifiedIdentity, String, Option<String>) {
        (self.identity, self.id_token, self.refresh_token)
    }
}

/// One native worker owns this listener and attempt. Dropping it closes the callback.
/// It is never shared with site pages, discovery, or the synchronization listener.
pub struct DesktopLogin {
    listener: TcpListener,
    redirect: RedirectUrl,
    url: String,
    state: CsrfToken,
    nonce: String,
    pkce: Option<PkceCodeVerifier>,
}

struct Grant {
    code: AuthorizationCode,
    pkce: PkceCodeVerifier,
}

fn client(redirect: &RedirectUrl) -> Client {
    // Signature verification runs through Verifier after exchange, never through this client.
    CoreClient::new(
        ClientId::new(DESKTOP_CLIENT_ID.into()),
        IssuerUrl::new("https://accounts.google.com".into()).expect("constant Google issuer"),
        CoreJsonWebKeySet::new(Vec::new()),
    )
    .set_auth_uri(AuthUrl::new(AUTH_URL.into()).expect("constant Google authorization URL"))
    .set_token_uri(TokenUrl::new(TOKEN_URL.into()).expect("constant Google token URL"))
    .set_auth_type(AuthType::RequestBody)
    .set_redirect_uri(redirect.clone())
}

impl DesktopLogin {
    /// `nonce` is captured by the native attempt, never supplied by a page or remote peer.
    pub fn bind(nonce: &str) -> Result<Self, AuthError> {
        if !(32..=128).contains(&nonce.len())
            || !nonce
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(AuthError::Invalid);
        }
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| AuthError::Unavailable)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| AuthError::Unavailable)?;
        let address = listener.local_addr().map_err(|_| AuthError::Unavailable)?;
        let redirect =
            RedirectUrl::new(format!("http://{address}/")).map_err(|_| AuthError::Invalid)?;
        let (challenge, pkce) = PkceCodeChallenge::new_random_sha256();
        let login_nonce = Nonce::new(nonce.into());
        let (url, state, _) = client(&redirect)
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                move || login_nonce,
            )
            .add_scope(Scope::new("email".into()))
            .add_extra_param("prompt", "select_account")
            .add_extra_param("access_type", "offline")
            .set_pkce_challenge(challenge)
            .url();
        Ok(Self {
            listener,
            redirect,
            url: url.into(),
            state,
            nonce: nonce.into(),
            pkce: Some(pkce),
        })
    }

    pub fn authorization_url(&self) -> &str {
        &self.url
    }

    pub fn complete(
        mut self,
        cancelled: &AtomicBool,
        verifier: &Verifier,
    ) -> Result<Credentials, AuthError> {
        let grant = self.wait(cancelled, SIGN_IN_TIMEOUT)?;
        if cancelled.load(Ordering::Acquire) {
            return Err(AuthError::Cancelled);
        }
        let response = client(&self.redirect)
            .exchange_code(grant.code)
            .set_pkce_verifier(grant.pkce)
            .request(&oauth_http)
            .map_err(|_| AuthError::Unavailable)?;
        let id_token = response.id_token().ok_or(AuthError::Invalid)?.to_string();
        let identity = verifier.verify(&id_token, &self.nonce, Platform::Desktop)?;
        if cancelled.load(Ordering::Acquire) {
            return Err(AuthError::Cancelled);
        }
        Ok(Credentials {
            identity,
            id_token,
            refresh_token: response.refresh_token().map(|token| token.secret().clone()),
        })
    }

    fn wait(&mut self, cancelled: &AtomicBool, timeout: Duration) -> Result<Grant, AuthError> {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if cancelled.load(Ordering::Acquire) {
                return Err(AuthError::Cancelled);
            }
            match self.listener.accept() {
                Ok((mut stream, peer)) => {
                    if !peer.ip().is_loopback() {
                        continue;
                    }
                    let result =
                        read_request(&mut stream).and_then(|request| self.callback(&request));
                    let accepted = matches!(
                        result,
                        Ok(_) | Err(AuthError::Denied | AuthError::ProviderRejected)
                    );
                    let message = if accepted {
                        "로그인 응답을 받았습니다. 앱으로 돌아가 주세요."
                    } else {
                        "잘못된 로그인 응답입니다."
                    };
                    let status = if accepted {
                        "200 OK"
                    } else {
                        "400 Bad Request"
                    };
                    let response = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{message}", message.len());
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
                    let _ = stream.write_all(response.as_bytes());
                    if accepted {
                        return result;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50))
                }
                Err(_) => return Err(AuthError::Unavailable),
            }
        }
        Err(AuthError::TimedOut)
    }

    fn callback(&mut self, request: &str) -> Result<Grant, AuthError> {
        let host = self
            .listener
            .local_addr()
            .map_err(|_| AuthError::Unavailable)?
            .to_string();
        let mut lines = request.split("\r\n");
        let parts: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
        if parts.len() != 3
            || parts[0] != "GET"
            || parts[2] != "HTTP/1.1"
            || !parts[1].starts_with('/')
        {
            return Err(AuthError::Invalid);
        }
        let mut hosts = 0;
        for line in lines.take_while(|line| !line.is_empty()) {
            let (name, value) = line.split_once(':').ok_or(AuthError::Invalid)?;
            if name.eq_ignore_ascii_case("host") {
                hosts += 1;
                if value.trim() != host {
                    return Err(AuthError::Invalid);
                }
            }
            if name.eq_ignore_ascii_case("origin")
                || name.eq_ignore_ascii_case("transfer-encoding")
                || (name.eq_ignore_ascii_case("content-length") && value.trim() != "0")
            {
                return Err(AuthError::Invalid);
            }
        }
        if hosts != 1 {
            return Err(AuthError::Invalid);
        }
        let target = url::Url::parse(&format!("http://{host}{}", parts[1]))
            .map_err(|_| AuthError::Invalid)?;
        if target.path() != "/" || target.fragment().is_some() {
            return Err(AuthError::Invalid);
        }
        let mut params = BTreeMap::new();
        for (name, value) in target.query_pairs() {
            if params
                .insert(name.into_owned(), value.into_owned())
                .is_some()
            {
                return Err(AuthError::Invalid);
            }
        }
        if params.get("state") != Some(self.state.secret()) {
            return Err(AuthError::Invalid);
        }
        if let Some(error) = params.get("error").filter(|_| !params.contains_key("code")) {
            self.pkce.take().ok_or(AuthError::Invalid)?;
            return Err(if error == "access_denied" {
                AuthError::Denied
            } else {
                AuthError::ProviderRejected
            });
        }
        let code = params
            .get("code")
            .filter(|code| {
                !code.is_empty() && code.len() <= 4096 && code.bytes().all(|b| b.is_ascii_graphic())
            })
            .ok_or(AuthError::Invalid)?;
        if params.contains_key("error") {
            return Err(AuthError::Invalid);
        }
        Ok(Grant {
            code: AuthorizationCode::new(code.clone()),
            pkce: self.pkce.take().ok_or(AuthError::Invalid)?,
        })
    }
}

fn read_request(stream: &mut TcpStream) -> Result<String, AuthError> {
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|_| AuthError::Unavailable)?;
    let started = Instant::now();
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    while started.elapsed() < Duration::from_secs(1) && request.len() < MAX_HEADER_BYTES {
        match stream.read(&mut buffer) {
            Ok(0) => return Err(AuthError::Invalid),
            Ok(n) => {
                request.extend_from_slice(&buffer[..n]);
                if request.len() > MAX_HEADER_BYTES {
                    return Err(AuthError::Invalid);
                }
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    return String::from_utf8(request[..end + 4].to_vec())
                        .map_err(|_| AuthError::Invalid);
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return Err(AuthError::Invalid),
        }
    }
    Err(AuthError::Invalid)
}

fn oauth_http(request: HttpRequest) -> Result<HttpResponse, AuthError> {
    if request.uri().to_string() != TOKEN_URL
        || request.method().as_str() != "POST"
        || request.body().len() > MAX_HEADER_BYTES
    {
        return Err(AuthError::Invalid);
    }
    let mut outbound = crate::http::Request::post(TOKEN_URL, request.body());
    outbound.timeout_ms = 5000;
    outbound.follow_redirects = false;
    outbound.headers = request
        .headers()
        .iter()
        .map(|(name, value)| {
            value
                .to_str()
                .map(|value| (name.as_str().into(), value.into()))
                .map_err(|_| AuthError::Invalid)
        })
        .collect::<Result<_, _>>()?;
    let response = crate::http::send(outbound).map_err(|_| AuthError::Unavailable)?;
    if (300..400).contains(&response.status) {
        return Err(AuthError::Invalid);
    }
    let status = response.status;
    let content_type = response.content_type.clone().unwrap_or_default();
    let bytes = response
        .bytes(MAX_TOKEN_RESPONSE + 1)
        .map_err(|_| AuthError::Unavailable)?;
    if bytes.len() > MAX_TOKEN_RESPONSE {
        return Err(AuthError::Invalid);
    }
    openidconnect::http::Response::builder()
        .status(status)
        .header("Content-Type", content_type)
        .body(bytes)
        .map_err(|_| AuthError::Invalid)
}

#[cfg(test)]
#[path = "google_desktop_tests.rs"]
mod tests;
