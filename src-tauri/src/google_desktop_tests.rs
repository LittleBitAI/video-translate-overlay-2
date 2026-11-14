use super::*;

const NONCE: &str = "9d53c2d19d3b82ed94901d58a24dab623191dad252a7eb36730dcfb75915b515";
const SECRET: &str = "offline-publisher-fixture";

fn bind(nonce: &str) -> Result<DesktopLogin, AuthError> {
    DesktopLogin::bind_with_secret(nonce, SECRET)
}

fn request(login: &DesktopLogin, query: &str) -> String {
    format!(
        "GET /?{query} HTTP/1.1\r\nHost: {}\r\n\r\n",
        login.listener.local_addr().unwrap()
    )
}

#[test]
fn callback_is_armed_before_the_browser_url_and_uses_fresh_state_and_pkce() {
    let login = bind(NONCE).unwrap();
    let other = bind(NONCE).unwrap();
    let url = url::Url::parse(login.authorization_url()).unwrap();
    assert_eq!(url.scheme(), "https");
    assert_eq!(url.host_str(), Some("accounts.google.com"));
    assert_eq!(url.path(), "/o/oauth2/v2/auth");
    let params: BTreeMap<_, _> = url.query_pairs().collect();
    assert_eq!(params["client_id"], DESKTOP_CLIENT_ID);
    assert_eq!(params["response_type"], "code");
    assert_eq!(params["code_challenge_method"], "S256");
    assert_eq!(params["nonce"], NONCE);
    assert!(!params.contains_key("client_secret"));
    assert!(!login.authorization_url().contains(SECRET));
    let scopes: std::collections::BTreeSet<_> = params["scope"].split_whitespace().collect();
    assert_eq!(
        scopes,
        std::collections::BTreeSet::from(["openid", "email"])
    );
    assert_ne!(login.state.secret(), other.state.secret());
    assert_ne!(
        login.pkce.as_ref().unwrap().secret(),
        other.pkce.as_ref().unwrap().secret()
    );
    let redirect = url::Url::parse(&params["redirect_uri"]).unwrap();
    assert_eq!(redirect.host_str(), Some("127.0.0.1"));
    assert_eq!(
        redirect.port(),
        Some(login.listener.local_addr().unwrap().port())
    );
    let address = login.listener.local_addr().unwrap();
    drop(login);
    assert!(
        TcpListener::bind(address).is_ok(),
        "dropping an attempt closes its listener"
    );
}

#[test]
fn invalid_callbacks_leave_the_pending_attempt_usable_and_valid_codes_are_consumed_once() {
    let mut login = bind(NONCE).unwrap();
    let query = format!("code=fixture-code&state={}", login.state.secret());
    let valid = request(&login, &query);
    let host = login.listener.local_addr().unwrap().to_string();
    for invalid in [
        request(&login, "code=fixture-code&state=wrong"),
        request(&login, &format!("{query}&state=duplicate")),
        request(&login, &format!("{query}&code=duplicate")),
        request(&login, &format!("{query}&error=access_denied")),
        valid.replace(&format!("Host: {host}"), "Host: example.org"),
        valid.replace("\r\n\r\n", &format!("\r\nHost: {host}\r\n\r\n")),
        valid.replace("\r\n\r\n", "\r\nOrigin: https://example.org\r\n\r\n"),
        valid.replace("\r\n\r\n", "\r\nTransfer-Encoding: chunked\r\n\r\n"),
        valid.replace("\r\n\r\n", "\r\nContent-Length: 12\r\n\r\n"),
        valid.replace("GET /?", "POST /?"),
        valid.replace("GET /?", "GET /other?"),
    ] {
        assert!(matches!(login.callback(&invalid), Err(AuthError::Invalid)));
        assert!(login.pkce.is_some());
    }
    let grant = login.callback(&valid).unwrap();
    assert_eq!(grant.code.secret(), "fixture-code");
    assert!(
        matches!(login.callback(&valid), Err(AuthError::Invalid)),
        "callback replay must not create another grant"
    );
}

#[test]
fn denial_and_provider_errors_consume_only_the_matching_attempt() {
    for (error, expected) in [
        ("access_denied", AuthError::Denied),
        ("server_error", AuthError::ProviderRejected),
    ] {
        let mut login = bind(NONCE).unwrap();
        let callback = request(
            &login,
            &format!("error={error}&state={}", login.state.secret()),
        );
        assert!(matches!(login.callback(&callback), Err(value) if value==expected));
        assert!(login.pkce.is_none());
        assert!(matches!(login.callback(&callback), Err(AuthError::Invalid)));
    }
}

#[test]
fn actual_loopback_callback_returns_no_code_state_or_credentials_to_the_browser() {
    let mut login = bind(NONCE).unwrap();
    let address = login.listener.local_addr().unwrap();
    let callback = request(
        &login,
        &format!("code=fixture-code&state={}", login.state.secret()),
    );
    let state = login.state.secret().clone();
    let browser = std::thread::spawn(move || {
        let mut socket = TcpStream::connect(address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        socket.write_all(callback.as_bytes()).unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("Cache-Control: no-store"));
        assert!(!response.contains("fixture-code"));
        assert!(!response.contains(&state));
        assert!(!response.contains("Access-Control-Allow-Origin"));
    });
    let grant = login
        .wait(&AtomicBool::new(false), Duration::from_secs(3))
        .unwrap();
    assert_eq!(grant.code.secret(), "fixture-code");
    browser.join().unwrap();
}

#[test]
fn cancellation_timeouts_and_oversized_callbacks_release_the_attempt() {
    let mut login = bind(NONCE).unwrap();
    assert!(matches!(
        login.wait(&AtomicBool::new(true), Duration::from_secs(3)),
        Err(AuthError::Cancelled)
    ));
    assert!(matches!(
        login.wait(&AtomicBool::new(false), Duration::from_millis(1)),
        Err(AuthError::TimedOut)
    ));
    let address = login.listener.local_addr().unwrap();
    let browser = std::thread::spawn(move || {
        let mut socket = TcpStream::connect(address).unwrap();
        let _ = socket.write_all(&vec![b'A'; MAX_HEADER_BYTES + 1]);
    });
    let (mut socket, _) = loop {
        match login.listener.accept() {
            Ok(value) => break value,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::yield_now(),
            Err(e) => panic!("{e}"),
        }
    };
    assert!(matches!(read_request(&mut socket), Err(AuthError::Invalid)));
    browser.join().unwrap();
    drop(socket);
    drop(login);
    assert!(TcpListener::bind(address).is_ok());
    assert!(matches!(bind("short"), Err(AuthError::Invalid)));
}

#[test]
fn exchange_sends_publisher_registration_only_to_token_endpoint_and_classifies_safe_errors() {
    for (body, expected) in [
        (
            r#"{"error":"invalid_request","error_description":"client_secret is missing. private-value"}"#,
            AuthError::Registration,
        ),
        (
            r#"{"error":"invalid_client","error_description":"private-value"}"#,
            AuthError::Registration,
        ),
        (
            r#"{"error":"invalid_grant","error_description":"private-value"}"#,
            AuthError::ProviderRejected,
        ),
        (
            r#"{"error":"invalid_request","error_description":"other private-value"}"#,
            AuthError::ProviderRejected,
        ),
        ("private-value-not-json", AuthError::ProviderRejected),
    ] {
        let login = bind(NONCE).unwrap();
        let result = client(&login.redirect, &login.client_secret)
            .exchange_code(AuthorizationCode::new("fixture-code".into()))
            .set_pkce_verifier(PkceCodeVerifier::new("fixture-verifier".into()))
            .request(&|request: HttpRequest| -> Result<HttpResponse, AuthError> {
                assert_eq!(request.uri().to_string(), TOKEN_URL);
                assert_eq!(request.method(), "POST");
                let fields: BTreeMap<_, _> = url::form_urlencoded::parse(request.body()).collect();
                assert_eq!(fields["client_id"], DESKTOP_CLIENT_ID);
                assert_eq!(fields["client_secret"], SECRET);
                assert_eq!(fields["code_verifier"], "fixture-verifier");
                assert_eq!(fields["redirect_uri"], login.redirect.as_str());
                Ok(openidconnect::http::Response::builder()
                    .status(400)
                    .header("Content-Type", "application/json")
                    .body(body.as_bytes().to_vec())
                    .unwrap())
            })
            .map_err(exchange_error);
        assert_eq!(result.err().unwrap(), expected);
        assert!(!expected.to_string().contains("private-value"));
    }
    let login = bind(NONCE).unwrap();
    let result = client(&login.redirect, &login.client_secret)
        .exchange_code(AuthorizationCode::new("fixture-code".into()))
        .request(&|_: HttpRequest| -> Result<HttpResponse, AuthError> {
            Err(AuthError::Unavailable)
        })
        .map_err(exchange_error);
    assert_eq!(result.err().unwrap(), AuthError::Unavailable);
    if DESKTOP_CLIENT_SECRET.is_none() {
        assert!(matches!(
            DesktopLogin::bind(NONCE),
            Err(AuthError::Registration)
        ));
    }
}

#[test]
fn token_transport_refuses_arbitrary_destinations_methods_and_unbounded_bodies() {
    for request in [
        openidconnect::http::Request::builder()
            .method("POST")
            .uri("https://example.org/token")
            .body(Vec::new())
            .unwrap(),
        openidconnect::http::Request::builder()
            .method("GET")
            .uri(TOKEN_URL)
            .body(Vec::new())
            .unwrap(),
        openidconnect::http::Request::builder()
            .method("POST")
            .uri(TOKEN_URL)
            .body(vec![0; MAX_HEADER_BYTES + 1])
            .unwrap(),
    ] {
        assert!(matches!(oauth_http(request), Err(AuthError::Invalid)));
    }
}
