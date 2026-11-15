# Google sign-in registration for local-network sync

This is setup for the pending local-network synchronization feature. Registration
does not implement synchronization or establish acceptance. The current released
app still keeps separate device-local libraries.

## Cost and approved boundaries

Creating the Google account and project used for sign-in is free, as documented
in Google's [sign-in codelab](https://codelabs.developers.google.com/codelabs/sign-in-with-google-button).
Direct Google OAuth/OpenID Connect sign-in does not require a hosted identity
database or a per-user subscription. This task uses Google only for identity;
library data travels directly between Windows and Android on a reachable local
network.

The user approved registration on the condition that it is free. Leave Cloud
Billing unlinked. Do not start a paid trial, attach a payment method, enable
Firebase or Identity Platform, or provision compute or storage. If the console
requires payment for a selected operation, stop and identify the operation
before proceeding. Existing Gemini translation billing stays unchanged; no
translation call is needed to register or test sign-in.

The user also selected an explicit confirmation before importing the old local
library into an account, with the original data preserved, and will assist with
real-account and physical-device checks during implementation.

## Create the registration

The owner selected the following project and supplied all three client IDs on
2026-11-15. These are public registration identifiers, recorded from the owner.
User-assisted sign-in and restoration after restart succeeded on Windows and
the physical Android phone at `31ced1f`; see the
[native authentication evidence](local-network-sync.md#native-authentication-at-31ced1f).
Public audience and branding configuration remain unverified.

| Registration | Owner-supplied value |
| --- | --- |
| Project name | `1234` |
| Project ID | `gen-lang-client-0430697481` |
| Windows Desktop client ID | `1072382283165-lar1422vmd8fnouh4dlpr771gcc7lq4f.apps.googleusercontent.com` |
| Android client ID | `1072382283165-1fifmmjgf3k0ff15h1fe7aie4jdb1h5i.apps.googleusercontent.com` |
| Android Web audience client ID | `1072382283165-3e4i7184unnrqunbr212i42hbg8fpics.apps.googleusercontent.com` |

The CLI required reauthentication when inspecting this project. No cloud
settings were changed; billing, audience, and published branding remain
unverified. Do not enable or link billing for registration, or disable an
existing billing configuration without first establishing what depends on it.

The project owner performs these steps in the
[selected project's Google Auth Platform console](https://console.cloud.google.com/auth/overview?project=gen-lang-client-0430697481).
Use a dedicated project for this app unless the owner explicitly selects an
existing project. The locally configured `sprint-ai-01` project has not been
selected for this task and must not be repurposed automatically.

1. Select `gen-lang-client-0430697481`. The project name `1234` can stay; it is
   separate from the app name shown during sign-in. Do not create another
   project for the remaining clients.
2. Open Branding. Set the application name to `Video Translate Overlay`, and
   select the owner's actual support and developer contact email addresses.
3. Select an External audience. An Internal audience would exclude ordinary
   personal Google accounts.
4. Restrict Data Access to sign-in identity: `openid` and
   `https://www.googleapis.com/auth/userinfo.email`. Do not add Drive, YouTube,
   Gmail, or other data-access scopes.
5. Create the clients in the following table in this same project. Keep any
   downloaded credential JSON outside Git and chat.

| Client type | Suggested name | Configuration |
| --- | --- | --- |
| Desktop app | `VTO Windows` | Windows uses the system browser, PKCE, and a temporary loopback callback. No LAN OAuth callback. |
| Android | `VTO Android` | Package `dev.vto.overlay`; signing certificate SHA-1 below. |
| Web application | `VTO Android identity audience` | The Web client ID is required by Android Credential Manager's Google sign-in API. It does not require deploying a web server for this native flow. Do not add arbitrary origins or redirect addresses. |

The existing Android signing key was read locally on 2026-11-14. Its SHA-256
matches the signing-continuity requirement in the
[release procedure](../releases/procedure.md):

```text
Package: dev.vto.overlay
SHA-1: E6:C3:14:40:6E:E8:88:BD:46:F8:26:12:D3:64:40:B8:17:2B:51:C8
SHA-256: FA:6D:A2:23:3C:20:62:B4:8B:AF:20:B3:56:2E:AC:AC:CE:77:4D:A8:0F:2E:4B:B3:D2:0E:C3:A6:A5:27:EC:28
```

Certificate fingerprints and client IDs are public registration metadata. Never
share the keystore, private keys, passwords, client-secret files, refresh/access
tokens, ID tokens, Gemini keys, or browser cookies. A differently signed test APK
needs its own Android client; never replace the installed app's signing key to
make registration work.

## Public availability

Testing with two named test users is useful during development, but it does not
satisfy the requirement that ordinary Google accounts can use the app. Before
completion, configure the External app for production and inspect the actual
published audience and branding state. Do not report public availability from
an owner-only or test-user-only sign-in.

If Google requests a home page, privacy policy, domain ownership, or brand
verification, prepare the exact text and destinations before publication.
The owner supplies contact details and any required domain ownership. Do not
invent those details, buy a domain, deploy hosting, or request broader scopes.
Google's [brand-verification guide](https://developers.google.com/identity/protocols/oauth2/production-readiness/brand-verification)
describes the requirements and possible review delay. A submitted or pending
review is not an accepted public configuration.

## Implementation handoff and acceptance

The project and all three public client IDs are recorded above. The Windows
publisher build also needs the selected Desktop client's credential JSON.
Although Google's desktop guide labels `client_secret` optional, a diagnostic
request against this registration on 2026-11-15 returned HTTP 400,
`invalid_request`, with `client_secret is missing`. The original implementation
omitted this field and incorrectly classified the exchange failure as a
verification outage. The repaired exchange includes it and reports registration
rejection separately from network failure.

Download the existing Desktop client's JSON from the selected project's Clients
page and save it locally as `artifacts/google-oauth/desktop-client.json`. An
existing download elsewhere may instead be named by the build environment
variable `VTO_GOOGLE_DESKTOP_CLIENT_JSON`. Do not paste the file or its contents
into chat. If Google no longer supplies the original secret, report that state;
do not rotate or recreate a client automatically.

The build validates the `installed` client type, exact project and Desktop
client ID, a bounded secret, and UTF-8 without BOM. Missing or mismatched
configuration stops a Windows app build before opening another broken login.
The file is read only by the publisher's Windows app build. Android builds and
default contract/HTTP tools do not read or embed it. Generated native source
stays under ignored `target/`; values are never emitted as Cargo environment
directives or diagnostic messages. Do not commit the input or generated files.

Google treats installed desktop apps as public clients: embedded registration
values cannot be kept confidential. PKCE, state, nonce and signed-token checks
remain the account-security boundary; the shared Desktop client secret does not
authenticate a user or a LAN peer. These registrations belong to the app
publisher. Ordinary installed-app users sign in directly and must not create
projects or supply credential files or client IDs.

Windows sign-in must follow Google's
[desktop OAuth guide](https://developers.google.com/identity/protocols/oauth2/native-app).
Android sign-in must follow the
[Credential Manager guide](https://developer.android.com/identity/sign-in/credential-manager-siwg-implementation),
which explicitly requires the Web client ID. YouTube WebView login and its
cookies remain separate.

Registration must be followed by implementation and actual Windows/Android
acceptance with two Google accounts, including mismatched-account rejection,
screen-off background delivery, both transfer directions, offline reopening,
deletion after reconnection, account switching, and restart recovery. No such
sync acceptance has run yet. The full task and repository gate remain required.
