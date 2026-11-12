---
scope: project
severity: contract
triggers: ["테스트", "test", "빌드", "build", "게이트", "gate", "실행", "돌려", "검증", "CI"]
slots: []
sources: [.wiki/adapter.toml]
links: [project]
---

# There is no build, run or test command yet — do not report one as passed

Rule. Until application code lands, this repository has no build, launcher
or test command: `gate_cmd`, `live_cmd` and `server_stop` in
`.wiki/adapter.toml` are empty and there is no `verification.toml`. Do not
invent a command and report it as a passing check. The task that adds the
first code also adds its build and test command and records it in
`.wiki/project.md`; ask the user to fill the adapter slots, since that file
is owned by wiki-agent.

Why. The repository holds only `README.md`, `LICENSE` and scaffolding; any
command claimed today would have nothing to run against.

What goes wrong. A report says tests passed when nothing ran, and the first
real regression has no gate to catch it.
