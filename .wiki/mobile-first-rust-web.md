---
scope: project
severity: contract
triggers: ["모바일", "mobile", "안드로이드", "android", "갤럭시", "S21", "rust", "러스트", "프레임워크", "framework", "최적화"]
slots: []
sources: [README.md, docs/design/backend.md, src-tauri/README.md, tests/overlay/overlay.test.ts]
links: [job-survives-background, repository-gate, project]
---

# The stack is Rust with Tauri 2, and phone fit is checked at the S21 Ultra's 384×854 viewport, not on the device

Rule. The backend stays Rust with Tauri 2, with Svelte for the screens, as
the 스택 table in `docs/design/backend.md` records. Moving off Rust is the
user's decision. Mobile layout is accepted by the designed check T11,
`test_overlay_fits_phone_and_desktop_screens` in
`tests/overlay/overlay.test.ts`. It runs the overlay at the Galaxy S21
Ultra's 384×854 viewport and at 1280×720, and requires that the 영상감지
button and the subtitles are not cut off in either. The phone itself is not
part of the gate. `src-tauri/README.md` ("Not in this build yet") records
that a real S21 Ultra has never run the app; only the Android emulator has.
Do not claim device behaviour that was not observed, and do not hold back a
change that passes [[repository-gate]] for lack of a phone run.

Why. The original request (quoted in `docs/design/backend.md`) asks for a
Rust-based web framework for mobile performance and names the S21 Ultra as
the test phone. The design turns that into the viewport test above.

What goes wrong. If phone behaviour is reported as tested when only the
viewport check or the emulator ran, the record claims a run that never
happened.
