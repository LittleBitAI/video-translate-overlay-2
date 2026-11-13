---
scope: project
severity: preference
triggers: []
domain: ''
title: "Added the first version of the repository wiki for the task agent to read (no functional changes)."
pr: 2
merged: 2026-11-13
branch: "wiki-bootstrap"
---

# Added the first version of the repository wiki for the task agent to read (no functional changes).

What. Added a single-page structure guide document summarizing what the repository intends to create, what files currently exist and their respective roles, and the fact that there are no build, run, or test methods yet. - Location: `.wiki/project.md` - Added six pages of rules to follow, extracted from the requirements document. Transcribe audio directly and leave translation to text only, do not exceed a cost of around 1,000 KRW per hour of video, subtitles should be ahead of audio...

Why. This app overlays a screen on top of videos like YouTube, and when the video detection button is pressed, it transcribes the audio of foreign language technical lectures and displays them as Korean subtitles. There is no app code yet, only a requirements document, and this change creates the initial wiki for the automated task agent that will work in this repository to reference. By leaving core requirements such as cost caps, prevention of subtitle lag, and background persistence as rules before code is created, it reduces the risk of future work violating them unknowingly or reporting that non-existent tests have passed.

Source. PR #2 · `wiki-bootstrap`
