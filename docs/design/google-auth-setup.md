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
The owner later confirmed the External audience is `In production` after
completing the approved branding fields; see Public availability below.

| Registration | Owner-supplied value |
| --- | --- |
| Project name | `1234` |
| Project ID | `gen-lang-client-0430697481` |
| Windows Desktop client ID | `1072382283165-lar1422vmd8fnouh4dlpr771gcc7lq4f.apps.googleusercontent.com` |
| Android client ID | `1072382283165-1fifmmjgf3k0ff15h1fe7aie4jdb1h5i.apps.googleusercontent.com` |
| Android Web audience client ID | `1072382283165-3e4i7184unnrqunbr212i42hbg8fpics.apps.googleusercontent.com` |

The CLI required reauthentication when inspecting this project. No cloud
settings were changed by that inspection. The later publishing status is an
owner observation, not an authenticated CLI inspection. Do not enable or link
billing for registration, or disable an
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

On 2026-11-15 the owner initially reported `External` and `Testing` on the
selected project's Audience page. After approving publication and entering the
approved branding fields below, the owner explicitly reported `In production`.
These are owner observations, not an independently authenticated console
inspection. Two different real accounts authenticated on both native platforms
and passed the [account-isolation flows](local-network-sync.md#two-real-google-accounts-at-5f899a2)
using the same clients and identity scopes. No owner-only account allowlist is
implemented in the app.

Testing with two named test users alone does not demonstrate ordinary-account
access. The public configuration must also permit accounts outside a test-user
list. Google's current [app-state overview](https://developers.google.com/identity/protocols/oauth2/production-readiness/overview)
documents an exception: External apps requesting only basic identity scopes
can admit accounts outside that list even while in Testing. Do not infer an
owner-only restriction from the Testing label alone.
Production configuration remains the intended public channel; switching it is
an outward configuration change requiring the owner's approval. Branding and
Workspace administrator restrictions are separate from basic identity access.

To report the current configuration without changing it:

1. Open the project's [Audience page](https://console.cloud.google.com/auth/audience?project=gen-lang-client-0430697481).
2. Confirm the selected project is `1234`, ID `gen-lang-client-0430697481`.
3. Read `Publishing status` near the top: `Testing` or `In production`.
4. Read `User type`: `External` or `Internal`. Leave `Publish app` and other
   configuration actions untouched while reporting the existing values.

Audience means who may sign in. Data Access means which Google information the
app requests permission to use. Google's [audience help](https://support.google.com/cloud/answer/15549945?hl=en)
also documents the basic-identity exception; the project's actual state still
needs to be observed by its owner.

The prepared production change is limited to this existing project's publishing
status, from `Testing` to `In production`, retaining its existing clients and
basic identity scopes. It does not publish an APK, installer or GitHub Release.
The owner approved this exact change on 2026-11-15 by selecting
`Publish Google sign-in`. The later explicit `In production` report establishes
the owner-observed result, separately from that approval. No connected
Google Cloud management tool is available in this implementation session, and
the host blocked launching the console link. The project owner completes the
confirmation in the existing Google console session.

Use `Publish app` on the same Audience page, review the Google confirmation,
and report the resulting status. If Google requests more
branding, domain or policy information, report those fields before introducing
new public destinations or contact details.

The owner subsequently reported that Google blocked production publishing
until a valid app name, support email, homepage URL and privacy-policy URL are
provided on Branding. Publishing approval has not bypassed this requirement.
At that point the public source repository had no configured homepage or Pages
site. The owner then approved free GitHub Pages and the Google account currently
signed in to the Windows app as the public support email. That identity was
read from the native app and checked against the original-account acceptance
receipt before inserting the approved address into both pages.

The approved [homepage source](../branding/index.html) and
[privacy-policy source](../branding/privacy.html) replace the earlier drafts.
Only those two HTML pages, their shared CSS and `.nojekyll` belong in the
publishing tree. They contain no scripts, forms or library-upload functions.
This is a static information website, not a synchronization backend. The
approval permits these free public pages; it does not authorize a paid domain,
another hosting service, a new application release or hosted synchronization.

| Google Branding field | Approved value |
| --- | --- |
| App name | `Video Translate Overlay` |
| User support email | The public address displayed on the homepage, selected from the project's permitted email list |
| Application home page | `https://littlebitai.github.io/video-translate-overlay-2/` |
| Privacy policy | `https://littlebitai.github.io/video-translate-overlay-2/privacy.html` |
| Authorized domain | `littlebitai.github.io`, subject to Google's actual ownership verification |

`node tests/branding.test.mjs` checks both pages at 384, 400 and 1280 CSS pixels,
navigation in both directions, consistent support links, the approved type
scale, 44 px navigation targets, absence of active collection surfaces and a
private-file request returning 404. All six layouts passed without horizontal
overflow. The website reuses the existing C1 palette instead of introducing a
new color system. Unused whitespace remains empty. Private layout receipts and
the approved contact capture stay under ignored `artifacts/branding/`.

Google's [branding requirements](https://support.google.com/cloud/answer/15549049?hl=en)
require a public homepage on an owned verified domain, a matching privacy-policy
link on that same domain, and a monitored support address visible to users.
GitHub Pages is [available for public repositories on GitHub Free](https://docs.github.com/en/pages/quickstart).
The owner subsequently confirmed production publishing after configuring the
approved fields. This records the accepted publishing state and required console
configuration as reported by the owner; it does not claim an independently
inspected Search Console ownership record or a Google verified-brand badge.
An accessible Pages URL alone would not establish Google's acceptance.

### Published static pages and in-app privacy access

The four reviewed assets were published from a separate `gh-pages` tree at
`989a4ebcdf1e1a5bc28c16f9e87f693dcd17cc6b`. The application branch was not pushed.
The Pages creation request returned `409 already enabled`; a read immediately
confirmed the intended `gh-pages` root source, matching built commit and enforced
HTTPS, so no replacement or retry was needed. Anonymous HTTPS requests for all
four assets returned 200 with exact local SHA-256 matches. A real Chromium
session followed homepage-to-policy and policy-to-home links on the public site.
No site visitor authentication was used. Receipts are ignored under
`artifacts/branding/`; Pages deployment is not repository-gate verification.

The Google account settings include a privacy control independent of sign-in.
Native P18 opens only the fixed published policy URL through the platform
browser. It takes no URL argument, changes no account or video state, and is
not available to site pages or HTTP callers. Android uses its separate bounded
native action; its release-download URL validation is unchanged. Ordinary
browser screens use a normal HTTPS link. Open failures have a retryable fixed
message and do not expose platform diagnostics.

Focused checks passed: the policy API regression (1), Windows/Android command
restrictions (3), native-only HTTP regression (1), rebuilt-host settings contracts
(3), and Chromium Google-account/preferences files (22). The first UI run found
an older YouTube fixture expecting browser-open arguments without the required
account generation. Updating that exact expected request and response to the
current contract made the affected file pass; the assertion was retained.
The cleanup audit keeps the new policy boundary test and static-page check,
extends existing denial/recovery checks, and removes no tests. Its updated
inventory covers 37 changed or affected source files, including inline API
checks and the Android capability check.

### Native privacy access at ff30ca8

Windows built and ran source `ff30ca80e0acd1af7c0ec110705ae8dc5d94d881` in the
same owned disposable profile. Core and screen identities matched, and the
original Google enrollment restored without interactive sign-in. The executable
SHA-256 is `de72186bbfc1691697da60e4f4ab4bfa039072328d3e05e2d4a516345489e847`.
Clicking the actual settings privacy button opened the exact published HTTPS
policy URL, observed in the external browser address bar. The account generation
and last-opened video pointer remained unchanged. The native command's fixed
destination and absence of URL input are separately checked offline.

The initial Windows observation exceeded its legacy PowerShell time limit.
A later attempt reached the browser URL but then failed because the harness
tried to intercept Tauri's read-only `invoke` property. Removing that
interception and using the actual browser destination plus unchanged app state
produced the passing `windows-policy-native.json` receipt. This was a harness
repair; the app opener did not change between those observations.

Android's Rust build completed, followed by the documented Windows symlink
refusal. The freshly copied JNI library was hash-checked and Gradle packaging
passed. The 488,660,407-byte debug APK has SHA-256
`a223f624956fe674bd3a8bd85c596d2f2ba9f51704c224b6ba9e9fa0fbe05416`.
Its packaged stripped core matches the fresh output, all 962 entries exclude
the publisher Desktop secret, and its certificate retains SHA-256
`fa6da2233c2062b48baf20b3562eacacce774da80f2e4bb3d20ec3a6a527ec28`.
ADB initially reported no connected phone. That pre-update backup stopped at
its first read and no installation was attempted. After the owner reconnected
the phone, a new complete backup retained the installed APK, original library,
both account databases and sealed Gemini credential before `adb install -r -t`
updated the physical Galaxy S21+ without uninstalling it.

`node artifacts/branding/privacy-native.mjs android` then passed: the running
core and screen both identified `ff30ca8`, the original account restored without
sign-in, and the actual settings button opened the exact policy URL observed in
Chrome's address bar. The account generation and last-opened video remained
unchanged. `python artifacts/native-sync-20261115/phone-privacy-retention.py final`
reported all original library rows and the sealed Gemini credential preserved,
with 3 ready jobs and 0 active jobs. The native foreground sync service was
present after the update. Private receipts are
`artifacts/branding/android-policy-native.json` and
`artifacts/native-sync-20261115/phone-after-privacy-update/retention.json`.
Documentation-only descendants reuse this evidence while application and test
sources remain unchanged; they do not identify a newly stamped native build.
The final read-only `node artifacts/branding/final-native-health.mjs` check
confirmed both running cores and screens identify `ff30ca8`, both restored the
original account, and both report `connected` after the Android update and
privacy action. It checks source equivalence before reusing native build evidence.

The free static website and its approved public contact are the only added
hosting scope. No website endpoint receives app authentication or library
transfers, and no paid domain or synchronization service is authorized. The
owner-confirmed production publishing status follows the required branding
configuration. No repository gate or final verification was run; those checks
remain server-owned.

If Google requests a home page, privacy policy, domain ownership, or brand
verification, prepare the exact text and destinations before publication.
The owner supplies contact details and any required domain ownership. Do not
invent those details, buy a domain, deploy hosting beyond the approved static
Pages site, or request broader scopes.
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
deletion after reconnection, account switching, and restart recovery. The
[native evidence](local-network-sync.md#native-one-account-flows-at-fc0d072)
records one-account transfers, playback, deletion, Home delivery and restart
recovery. Later observations at `5f899a2` cover screen-off Ethernet/Wi-Fi delivery
and two-account isolation on both native platforms. The final privacy action
passed on both platforms at `ff30ca8`, and the owner confirmed public production
configuration. The server-owned repository gate and final verification remain
pending; the focused checks are not a claim that either passed.
