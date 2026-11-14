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

The project owner performs these steps in the
[Google Auth Platform console](https://console.cloud.google.com/auth/overview).
Use a dedicated project for this app unless the owner explicitly selects an
existing project. The locally configured `sprint-ai-01` project has not been
selected for this task and must not be repurposed automatically.

1. Create a project named `Video Translate Overlay`. Leave billing unlinked and
   record the generated project ID.
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

The implementation needs the project ID and the three public client IDs. The
Windows credential download stays local for any installed-client configuration
the implementation requires. These registrations belong to the app publisher;
ordinary app users must not create their own projects or supply client IDs.

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
