#!/bin/sh
# Starts, force-stops and relaunches the debug Windows app built from the
# checked-out head (`cargo build -p video-translate-overlay --features app
# --bin video-translate-overlay`). The app gets its own data folder and WebView2
# profile, so it shares nothing with an installed copy and has no Google
# account (no local-network sync). Gemini is the loopback stand-in; the desktop
# model is a cached copy (VTO_WHISPER_MODEL), so nothing is fetched or billed.
# The app's stderr and the stand-in's request log go next to the data folder.
# usage (from the repository root): D=<data dir> M=<model file> sh desktop-run.sh start|kill|relaunch
set -u
E=docs/evidence/transcription-speed
P=${P:-9333}   # WebView2 DevTools port
G=${G:-8138}   # stand-in Gemini port
APP=target/debug/video-translate-overlay.exe
now() { date -u +%Y-%m-%dT%H:%M:%S.%3NZ; }
launch() {
  VTO_APP_DATA_DIR="$D" VTO_WHISPER_MODEL="$M" WEBVIEW2_USER_DATA_FOLDER="$D-webview" \
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=$P" "$APP" 2>> "$D-app.log" &
}
case $1 in
  start)
    mkdir -p "$D"
    echo "http://127.0.0.1:$G" > "$D/gemini_base_url.txt"
    python -I $E/scripts/gemini-stand-in.py "$G" "$D-gemini.jsonl" &
    echo "# $(now) launched" >> "$D-app.log"
    launch ;;
  kill)
    echo "# $(now) force-stopped (taskkill /F)" >> "$D-app.log"
    taskkill //F //IM video-translate-overlay.exe ;;
  relaunch)
    echo "# $(now) relaunched with the same folders" >> "$D-app.log"
    launch ;;
esac
