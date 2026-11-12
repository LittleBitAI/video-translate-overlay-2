//! A media request only ever goes to an address that was checked: a name that
//! does not resolve is refused before any request, and every other URL carries
//! the address its request is pinned to (`http_pin_tests.rs` shows the pin holds).

use super::*;

#[test]
fn a_name_that_does_not_resolve_is_refused_before_any_request() {
    // `.invalid` never resolves (RFC 6761).
    for u in ["http://media.vto-test.invalid/a.m4a", "https://media.vto-test.invalid/a.m4a"] {
        assert_eq!(checked(u, false).unwrap_err().code, "unreachable", "{u}");
        assert_eq!(get(u, None, false, 2_000).err().map(|r| r.code), Some("unreachable"), "{u}");
    }
}

#[test]
fn every_checked_url_carries_the_address_its_request_is_pinned_to() {
    let (_, addr) = checked("https://93.184.215.14/a.m4a", false).unwrap();
    assert_eq!(addr, Some("93.184.215.14:443".parse().unwrap()));
    let (_, addr) = checked("http://[2606:2800::1]:8080/a.m4a", false).unwrap();
    assert_eq!(addr, Some("[2606:2800::1]:8080".parse().unwrap()));
    for u in ["http://example.com/a.m4a", "https://example.com/a.m4a"] {
        if let Ok((_, addr)) = checked(u, false) {
            assert!(addr.is_some_and(|a| !is_private(a.ip())), "{u}: {addr:?}");
        } // Offline: the name does not resolve, which the test above covers.
    }
}

#[test]
#[ignore = "needs the network"]
fn a_real_https_file_is_probed_through_the_pin() {
    // GitHub redirects to its raw content host; each hop is checked and pinned.
    let url = Url::parse("https://github.com/ggerganov/whisper.cpp/raw/master/samples/jfk.mp3").unwrap();
    assert!(matches!(probe(&url, false), Ok(Source::File { .. })));
}
