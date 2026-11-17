# Automatic fullscreen subtitle sizing

Application revision: `aedada2`. On 2026-11-17 the user selected
"Accept readability (Recommended)" after receiving the committed short- and
long-cue Windows fullscreen captures. This accepts automatic sizing at 57.6 px
for a 1920×1080 video area. The accepted captures are
`native-fullscreen-short.png` and `native-fullscreen-long.png`; their hashes are
in `captures.json`. These are focused execution checks; the server-owned
repository gate and final hosted CI have not been run by this session.

The packet above describes the accepted implementation. For the reviewed
candidate, use the fresh, source-bound packet under "Review round 1" below.
The original receipt is retained as history and is not current runtime evidence.

## Player and build

The installed baseline is Windows 0.1.8 at
`%LOCALAPPDATA%/video-translate-overlay/video-translate-overlay.exe`, SHA-256
`2cc473e758178127b7f28ce55e3f2587bd8fe65672f300f949ce4e3f74f504df`.
Desktop `TauriBrowser::navigate` opens the separate `site` window. It injects
`src/overlay/inject.ts`, which mounts `src/overlay/index.ts` on the main video.
That renderer had the 26 px ceiling. The local Svelte screen instead uses
`Stage.svelte` and `Overlay.svelte`; its former viewport breakpoints did not
follow the video size.

The tested preview is a real Windows app built with:

```powershell
$env:LIBCLANG_PATH='C:/Users/dasdk/AppData/Local/Programs/Python/Python310/Lib/site-packages/clang/native'
cargo build -p video-translate-overlay --features app --bin video-translate-overlay
```

Result: `Finished dev profile [unoptimized + debuginfo] target(s) in 27.91s`.
This also bundles both the injected overlay and the Svelte screens through
Vite. The linker emitted its usual import-library warning; the build passed.
The environment was Windows 11 Home build 26200, WebView2 157.0.4341.46,
Node 24.18.0 and Playwright 1.61.0.

`receipt.json` records the native core/screen revision, binary SHA-256,
disposable profile digest, debugging listener ancestry, actual native window
geometry, and measured DOM bounds. The browser run used the same built screens
and the existing real `vto-web` core at `dc161893`; the collector verifies that
its Rust/Cargo inputs have no differences from the application revision.
The captures' hashes are in `captures.json`.

## Actual Windows playback

An isolated native profile contains synthetic saved Korean cues, seeded with the
existing verification helper. The player serves the repository's three-second
black MP4 fixture locally. The app injects its production bundle and loads the
saved cues through real Tauri IPC. There is no replacement overlay or subtitle
request handler. No translation or transcription was started, and the installed
user profile was not modified.

| Action/state | Actual viewport | Visible image | Subtitle size |
| --- | --- | --- | --- |
| Restored, short cue | 1280×800 | 1280×720 | 38.4 px |
| Fullscreen, short and long cues | 1920×1080 | 1920×1080 | 57.6 px |
| Fullscreen, letterboxed long cue | 1920×1080 | 1536×864 | 46.08 px |
| Fullscreen exit, long cue | 1280×800 | 1280×720 | 38.4 px |
| Native window resize, long cue | 960×600 | 960×540 | 28.8 px |

The app's own fullscreen button entered and exited fullscreen. Native UI
Automation resized the actual player window. A backward seek returned the short
cue; seeking forward returned the long cue. Actual playback advanced the source
clock, and pause kept it fixed. Korean wrapping and an explicit newline stayed
inside the visible image, with clearance above the native playback controls.
The subtitle response before and after every action was identical; its digest is
`6b8536b72f7e282156f9999155df69b246f7dc53ff5d9003df14f6c780f28696`.

## Local screen and phone fit

The browser run uses the production Svelte player and real local API with an
isolated stand-in profile. Only the existing synthetic saved cues are read.

| Browser viewport | Local subtitle size | Observations |
| --- | --- | --- |
| 1280×720 | 33.4392 px | Fits video; no page overflow |
| 1920×1080 | 52.6392 px | Fits video; no page overflow |
| 384×854 | 14 px | Fits video above controls; phone menu remains visible |

The first phone capture exposed the old local overlay's overlap with the seek
bar. Both renderers now reserve at least 48 px below subtitles, increasing to
10% of image height in larger players. The retained captures and receipt were
collected after this final code change.

## Focused checks and test cleanup

After the final code change, the affected checks passed:

```text
npx vitest run tests/overlay/overlay.test.ts tests/overlay/placement.test.ts tests/overlay/frames.test.ts tests/overlay/hidden_mount.test.ts tests/overlay/trusted_types.test.ts
Test Files 5 passed (5); Tests 43 passed (43)

npx vitest run -c vitest.ui.config.ts tests/ui/subtitles.test.svelte.ts tests/ui/watch.test.ts
Test Files 2 passed (2); Tests 41 passed (41)

node docs/evidence/fullscreen-subtitles/scripts/check.mjs
PASS: 7 actual Windows states and 3 browser viewports; saved cues unchanged
```

The new geometry checks exercise both renderers, including 4:3 and wide images,
short and long Korean cues, explicit newlines, wrapping, restore and resize.
The existing seek/pause, placement, hidden-page, frame and watch-screen checks
still guard current behavior. The cleanup audit found no obsolete tests,
superseded assertions, duplicate observations or stale skips to remove.

To reproduce the captures, run from the repository root with dependencies and
the native binary already built. Choose unused local ports 8157, 8159 and 9251,
and a fresh `artifacts/subtitle-scaling/windows-final` directory. Start
`node docs/evidence/fullscreen-subtitles/scripts/server.mjs`. Set
`WIKI_HOST_TOOL` to this wiki-agent installation's `tool` directory, then run
`python docs/evidence/fullscreen-subtitles/scripts/launch.py` for the isolated
native preview. Seed a separate browser profile with
`python scripts/verification/seed.py artifacts/subtitle-scaling/browser-data 127.0.0.1 8159`,
then run `target/debug/vto-web.exe --stand-ins` with `PORT=8157` and
`APP_DATA_DIR` pointing to that profile. Run the collector command above.
Creating `artifacts/subtitle-scaling/windows-final/stop` closes the owned native
process tree. Stop only the two servers started for this run.

The public repository has an active Windows CI workflow, so final verification
belongs to hosted CI under the server's control. These checks do not claim that
gate. No release installer, live YouTube session or physical Android device was
tested; no release was published.

## Review round 1: source-bound runtime and offline evidence

The review refused the original packet because its receipt identified an older
revision and did not retain a binding to the reviewed `f68351a` candidate. The
repair changes verification only. The application and existing subtitle tests
are unchanged from the user's accepted rendering; `review-1/source-binding.json`
records the compared paths, revisions and empty application/test diff.

The fresh producer is `be8b00b36acd794630d6a3f9d782778d40e9a776`.
Both Windows and browser executables were rebuilt from that commit. The native
core, native screens, browser core, browser screens and runtime collector all
report that same revision in `review-1/runtime.json`. Its environment, executable
hashes, isolated profile digest and both debugging/server listener ancestries
are retained alongside seven actual Windows states and three browser viewports.
The same fullscreen, resize, seek, pause, Korean wrapping, control-clearance and
unchanged-cue assertions were collected again. Runtime and offline results from
the original packet are not reused.

`scripts/verification/subtitle-binding.mjs` records a digest of the producer's
tracked Git tree. It excludes only wiki metadata, this acceptance document and
the `review-1` output directory. Application code, tests, manifests, locks,
build scripts, fixture helpers, runtime collectors and the binding/audit scripts
remain included. The read-only audit recomputes that tree at the producer and
at the candidate's current HEAD. It refuses missing binding data, altered
digests and any change to an included input. An evidence-only commit can
therefore retain results without claiming its executables were restamped.
The packet does not attempt to contain its own future commit identifier.

The binding regression test rejects the original unbound receipt. A table of
revision and receipt forms checks acceptance of unchanged inputs and rejection
of mismatched inputs; a second table checks that executable verification scripts
and test files remain inside the binding scope. Run it with
`node --test scripts/verification/subtitle-binding.test.mjs`.

`review-1/focused.json` pins the same producer, exact commands, exit statuses
and hashes of the retained logs: three binding tests, 43 injected-overlay tests
and 41 local-screen tests passed. The test cleanup audit retains these checks
because the binding, renderer geometry and source-clock behavior are distinct
current regressions; no tests or stale skips were removed.

The first packet-audit attempt exposed a reader error: PowerShell reported one
listener as a string, while the audit expected an array. That failed attempt is
preserved under `review-1/attempt-1`, including its source identity and failure
record. The reader now normalizes both forms before checking the single allowed
loopback address. Final builds, focused tests and runtime captures were produced
again after that last verifier change.

Run `node scripts/verification/subtitle-evidence-audit.mjs` on the candidate to
check the source binding, runtime producer identities, retained geometry,
listener ownership, ten capture hashes and all focused log hashes. Its output
names both the candidate HEAD and the producer revision. The actual audit result
is retained in `review-1/audit.log`.

For a fresh execution, the earlier reproduction steps still apply. Set
`SUBTITLE_OUTPUT=artifacts/subtitle-scaling-review-1-final` for both the native
launcher and runtime collector and seed the browser profile under that same
directory. A new output directory avoids overwriting a previous native profile.

`review-1/source-binding.json` distinguishes reused user acceptance from fresh
execution evidence and the server owner's earlier gate report at `f68351a`.
The repair does not claim that earlier gate for the repaired candidate and does
not dispatch final verification. The server owns the next checks and final
hosted CI. Live YouTube, a physical phone, installer packaging and release
publication remain outside these focused runs.

## Review round 2: retain the comparison with the reviewed candidate

The previous retained audit log named only the producer's own revision. A later
console audit had compared the evidence with the candidate, but that output was
not committed. Round 2 retained an actual audit execution for reviewed commit
`8e53d39bf066e0c92d8011f351a5ac46115dcd00`. That record remains in Git history.
The current `review-1/audit.log` and `review-1/source-binding.json` identify the
latest reviewed candidate described below and explicitly mark the runtime and
offline runs as reused from `be8b00b`.

`review-1/reviewed-head.json` retains the execution time, current environment,
producer and candidate input digests, the exact excluded changes, reused build
identities, and hashes of the fresh audit and binding-test output. Both input
digests are identical. The three binding regression tests passed again; the
runtime, browser, desktop, API and 84 renderer-test observations are reused,
not presented as fresh executions. User readability acceptance is also reused.
The earlier server gate report remains an owner report for its original commit,
not a gate pass for this repair.

After each evidence-only correction, the implementation runs the audit again
after committing and names that final HEAD in its completion report. The
committed log records the reviewed candidate; the post-commit report records
the new evidence-only commit. This avoids claiming that a committed file can
contain its own future commit identifier. No application or verifier source
changed in this round, and final verification remains server-owned.

## Review round 4: bind the merge-maintenance candidate

Merge maintenance advanced the candidate from `5ce5fed` to
`31b5d9ebe5fb26005909b520e3513aff5751cf41`. The exact diff contains only
`.wiki/corpus.json` and `.wiki/graph.json`. Both are excluded by the existing
source-binding policy; no application, test, fixture, build or verifier input
changed. The previous retained audit still named the round 2 candidate, so it
did not record the comparison with this maintenance commit.

The current retained audit now names `31b5d9e`. `review-1/reviewed-head.json`
records an actual successful execution at that commit, the two changed paths,
identical input digests, environment, reused executable identities, and fresh
audit/test-log hashes. All three binding tests passed again. Runtime, API,
browser, desktop and 84 renderer-test observations remain explicitly reused
from `be8b00b`; user readability acceptance is also reused. The earlier gate
result is only an owner report for its original revision.

The same binding policy applies to the collector, native core, native screen,
browser core, browser screen and offline test producer. Maintenance commits
require a new recorded comparison even when their inputs are excluded. After
this evidence-only commit, the implementation audits its final HEAD and returns
a matching completion report. No application or verifier code changed, no
runtime scenarios were repeated, and final verification remains server-owned.
