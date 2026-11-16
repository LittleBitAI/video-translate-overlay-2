#!/bin/sh
# Times an in-app writing stage in the background at no cost: the app must already
# be writing (pressed, then sent Home). Near the last window the phone's network is
# turned off, so the glossary request that follows writing cannot reach Gemini;
# after the last window the app is stopped and the network restored.
# usage: writing-only.sh LABEL LAST_WINDOW_INDEX
ADB=${ADB:-adb}
export MSYS_NO_PATHCONV=1
T=${T:?folder holding the run files}
label=$1; last=$2
log=$T/$label.logcat
offline=0
while :; do
  sleep 5
  i=$(grep -a "I/vto" "$log" | grep -a -o "region i=[0-9]*" | tail -1 | cut -d= -f2)
  [ -z "$i" ] && continue
  if [ $offline = 0 ] && [ "$i" -ge $((last - 3)) ]; then
    $ADB shell cmd wifi set-wifi-enabled disabled; $ADB shell svc data disable
    offline=1; echo "offline at window $i $(date +%T)"
  fi
  if [ "$i" -ge "$last" ]; then
    $ADB shell am force-stop dev.vto.overlay; echo "stopped after window $i $(date +%T)"
    break
  fi
  if grep -a -q "glossary\|translat" "$log" && grep -a "I/vto" "$log" | grep -a -q "glossary ms="; then
    $ADB shell am force-stop dev.vto.overlay; echo "stopped at glossary $(date +%T)"; break
  fi
done
sleep 2
$ADB shell cmd wifi set-wifi-enabled enabled; $ADB shell svc data enable
touch "$T/stop-$label"
echo done
