//! A host name is only ever requested at an address that was checked: one that
//! does not resolve is refused before any request, never handed to the HTTP
//! client to resolve again on its own.

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
fn a_plain_http_name_always_goes_to_its_checked_address() {
    // Literal public addresses need no lookup; every resolved name carries the address it was checked at.
    let (url, addr) = checked("http://93.184.215.14/a.m4a", false).unwrap();
    assert_eq!((url.as_str(), addr), ("http://93.184.215.14/a.m4a", None));
    if let Ok((url, addr)) = checked("http://example.com/a.m4a", false) {
        let addr = addr.expect("a resolved name carries the address it was checked at");
        let (to, host) = pinned(&url, Some(addr));
        assert_eq!(to.host_str().unwrap().trim_matches(['[', ']']), addr.ip().to_string());
        assert_eq!(host.as_deref(), Some("example.com"));
    } // Offline: the name does not resolve, which the test above covers.
}
