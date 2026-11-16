#!/bin/sh
# Detached: waits for the PC base runs, then writes the large-model reference (desktop decoding, plan 2).
R=${R:?the artifacts folder}
while tasklist | grep -qi tt-pc-head; do sleep 30; done
sleep 5
while tasklist | grep -qi tt-pc-head; do sleep 30; done
"$R/tx-speed/tt-pc-head.exe" "$R/dl-speed/pc-c1/audio.m4a" "$R/models/ggml-large-v3-turbo-q5_0.bin" "$R/tx-speed/reference-large.json" --plan 2 > "$R/tx-speed/reference-large.log" 2>&1
echo "exit=$?" >> "$R/tx-speed/reference-large.log"
