# Offline signature fixture

`test-only-rsa.pem` is an intentionally public test private key, generated for
`google_auth_tests.rs`. It signs local JWT fixtures against a test JWKS supplied
directly to the verifier. It is not a Google credential or a production key.
Production verification fetches Google's public keys from its fixed HTTPS URL
and never trusts this fixture.
