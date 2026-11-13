---
scope: project
severity: contract
triggers: ["백그라운드", "background", "바탕화면", "홈 ?화면", "앱.*(나가|닫|꺼)", "알림", "notification", "영상감지"]
slots: []
sources: [README.md, docs/design/backend.md, src-tauri/README.md, contract-tests/tests/jobs.rs]
links: [mobile-first-rust-web, project]
---

# Once 영상감지 is pressed, the job finishes even if the user leaves the app

Rule. After the 영상감지 button starts a job, download, transcription and
translation keep running while the overlay is closed, the app is in the
background, or the user is on the home screen. The user gets a notification
when the subtitles are ready. Job state lives in SQLite through
`src-tauri/src/pipeline.rs`, never only in a page or a foreground view, so a
killed process resumes from what it last wrote. On Android the job runs
under the foreground service that `src-tauri/src/android.rs` starts when the
button is pressed (assumption A5 in `docs/design/backend.md`). The designed
check is T8, `test_job_advances_in_background_and_resumes` in
`contract-tests/tests/jobs.rs`.

Why. `README.md` and the original request quoted in `docs/design/backend.md`
require the work to continue in the background after detection, and make
the notification the end of the flow.

What goes wrong. On Android the OS suspends a backgrounded page or tab, so a
job held there silently stops and the user returns to nothing.
