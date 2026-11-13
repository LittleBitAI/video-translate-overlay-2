---
scope: project
severity: contract
triggers: ["테스트", "test", "빌드", "build", "게이트", "gate", "검증", "CI", "nextest", "vitest", "다 됐", "완료"]
slots: []
sources: [.wiki/adapter.toml, README.md, .github/workflows/ci.yml, scripts/contract-tests.mjs]
links: [project]
---

# A change is done only when the one-line repository gate passes — a subset is not the gate

Rule. The gate is the whole `gate_cmd` in `.wiki/adapter.toml`: `npm ci`,
then `node scripts/contract-tests.mjs artifacts/junit.xml` (the backend
contract tests, the Rust unit tests and the overlay tests), then
`npm run test:ui`, then `npm run build`. Never report a single
`cargo test`, a single Vitest file or a build alone as the gate passing. Name
the narrower command you actually ran. The 테스트 section of `README.md` and
`.github/workflows/ci.yml` run the same line, so a change to one of the three
is made in all three.

Why. `scripts/contract-tests.mjs` builds `vto-contract-host` and runs the
backend and overlay suites into one JUnit report. A failure in one tool does
not stop the other, and the script exits non-zero only after both have run.
The two screens' tests and the screen build are separate steps after it.
Each step covers code the others do not.

What goes wrong. A Rust-only run passes while the overlay or the screens are
broken. The report calls the change done, and CI on `windows-latest` finds
the failure after the push.
