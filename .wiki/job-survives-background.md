---
scope: project
severity: contract
triggers: ["백그라운드", "background", "바탕화면", "홈 ?화면", "앱.*(나가|닫|꺼)", "알림", "notification", "영상감지"]
slots: []
sources: [README.md]
links: [mobile-first-rust-web, project]
---

# Once 영상감지 is pressed, the job finishes even if the user leaves the app

Rule. After the 영상감지 button starts a job and the audio is downloaded,
transcription, translation and subtitle alignment must keep running while
the overlay is closed, the app is in the background, or the user is on the
home screen; the user is told by a notification when the subtitles are
ready. Do not keep pipeline state only in the page or the foreground view.

Why. `README.md` requires the work to continue in the background after
detection, and makes the notification the end of the flow.

What goes wrong. On Android the OS suspends a backgrounded page or tab, so a
job held there silently stops and the user returns to nothing.
