#!/bin/sh
# Runs the focused checks of the transcription-speed change and writes one
# receipt per check into receipts/: the source it ran on (commit and the Git
# tree ids of the directories that decide the result), the command, its exit
# status, and its test lines. Committing the receipts afterwards changes the
# commit but none of those tree ids, so `git rev-parse HEAD:src-tauri` (and the
# others) at any later commit shows whether a receipt still describes it.
# Local paths and the user's home are replaced by <repo> and <home>.
# usage (from the repository root): sh docs/evidence/transcription-speed/scripts/focused-checks.sh
set -u
E=docs/evidence/transcription-speed
OUT=$E/receipts
mkdir -p "$OUT"
SOURCES="src-tauri contract-tests src tests $E/scripts $E/data"
[ -z "$(git status --porcelain -- $SOURCES)" ] || { echo "uncommitted changes in the checked sources"; exit 1; }
root=$(pwd)
host="$root/target/debug/vto-contract-host"
[ -f "$host.exe" ] && host="$host.exe"

receipt() { # name, command...
  name=$1; shift
  f=$OUT/$name.txt
  started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  "$@" > "$OUT/.$name.log" 2>&1
  status=$?
  {
    echo "check: $name"
    echo "command: $*"
    echo "exit: $status"
    echo "started: $started"
    echo "finished: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "commit: $(git rev-parse HEAD)"
    for d in $SOURCES; do echo "tree $d: $(git rev-parse HEAD:$d)"; done
    echo "rustc: $(rustc --version)"
    echo "os: $(uname -s) $(uname -r)"
    echo "--- output (test, result and failure lines) ---"
    grep -E "^test |test result|panicked|^(ok|FAIL) |Running |error(\[|:)| (✓|×|✗) |Test Files|Tests  " "$OUT/.$name.log" \
      | sed -e 's/\x1b\[[0-9;]*m//g' -e "s#$root#<repo>#g" -e "s#$(cygpath -m "$root" 2>/dev/null || echo "$root")#<repo>#g" \
            -e "s#$HOME#<home>#g" -e 's#C:\\Users\\[^\\]*#<home>#g' -e 's#C:/Users/[^/]*#<home>#g' -e 's#[[:space:]]*$##'
  } > "$f"
  rm -f "$OUT/.$name.log"
  echo "$name exit=$status"
}

cargo build --workspace --bin vto-contract-host > /dev/null 2>&1 || { echo "contract host build failed"; exit 1; }
receipt rust-unit-audio-pipeline-stitch-gemini-store cargo test --manifest-path src-tauri/Cargo.toml --lib -- audio pipeline stitch gemini store
receipt rust-whisper-feature-builds cargo test --manifest-path src-tauri/Cargo.toml --lib --features whisper --no-run
export VTO_CONTRACT_HOST="$host"
receipt contract-jobs-translation-timing cargo test --manifest-path contract-tests/Cargo.toml --test jobs --test translation --test write_and_timing
receipt evidence-reproduce python -I $E/scripts/reproduce.py
receipt overlay-unsupported-cpu-notice npx vitest run tests/overlay/overlay.test.ts
receipt screen-unsupported-cpu-notice npx vitest run -c vitest.ui.config.ts tests/ui/watch.test.ts
