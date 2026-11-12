---
scope: project
severity: preference
triggers: []
slots: []
sources: []
links: []
---

# video-translate-overlay-2 — a Korean-subtitle overlay for foreign-language tech lectures, still at the spec stage

What it is for. The product described in `README.md` is a translation app
for foreign-language deep-learning and technical talks. It sits as an overlay
on a video site such as YouTube. When the user presses the 영상감지 (video
detect) button, the app downloads the video as audio, transcribes the speech
with its own speech-to-text model, translates the transcript with
`gemini-3.1-flash-lite`, aligns Korean subtitles to the audio with no drift
(밀림 현상), and notifies the user when they are ready. After detection the
job has to keep running while the user leaves for the home screen or puts the
app in the background. Two constraints come from the README: it must run well
on mobile, with a Galaxy S21 Ultra as the test device, which is why a
Rust-based web framework is asked for; and it must cost about 1,000 KRW per
hour of video. That cost target is the reason for transcribing locally and
translating text, instead of using Gemini's direct audio-to-Korean
subtitling. A later extension is voice conversion (for example with Qwen) so
the lecture is heard in a voice the user picks.

Current state. There is no application code yet. The repository holds the
spec and project scaffolding only, so there is nothing to build, run or test.
Do not assume a language, crate layout or framework beyond what the README
states; the first implementation task decides those.

Top-level layout.

| Path | Role |
| --- | --- |
| `README.md` | The product spec, in Korean: the user's original request quoted in full, with a one-paragraph summary above it. The only description of the intended architecture. |
| `LICENSE` | Apache License 2.0. |
| `.gitignore` | Generic ignores for several stacks (Python venvs and caches, `node_modules/`, Rust `target/`, `dist/`, `build/`, `.env`) plus `artifacts/`, the scratch directory for runs and review output. |
| `.gitattributes` | Forces `text=auto eol=lf`, so files are stored with LF line endings even on Windows. |
| `.claude/settings.json` | Claude Code permission denies: blocks in-place `perl -i` edits, `dos2unix`/`unix2dos`, `git reset --hard`, `git clean -fdx`, and Codex's `request_user_input_async`. |
| `.wiki/` | This wiki, managed by wiki-agent. `adapter.toml` declares the agents (`claude`, `codex`) and the slot values; `decisions/` holds one record per merged PR; `corpus.json` and `graph.json` index the repository's documents; `trajectory.jsonl` is a Git-ignored session log. |

How the parts will talk. Only the README's pipeline exists so far: overlay
button → audio download → local transcription → text translation by Gemini →
subtitle alignment → user notification, with the stages after the download
running as a background job. No interfaces, ports or processes are defined
yet.

Build, run and test. None exist. In `.wiki/adapter.toml` the `gate_cmd`,
`live_cmd` and `server_stop` slots are empty, and there is no
`verification.toml`. The only check in force is the wiki-agent lint run as
the task gate. Once code lands, fill those slots and update this page with the
real layout and commands.
