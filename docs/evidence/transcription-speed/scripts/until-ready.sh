#!/bin/sh
# Watches a full in-app run (started with start-run.sh) until translation ends:
# samples the app's cpuset and screen state every 60 s, then stops logcat and
# the thermal poll. usage: until-ready.sh LABEL
label=$1
export MSYS_NO_PATHCONV=1
ADB=${ADB:-adb}
T=${T:?folder holding the run files}
cd $T
until grep -a "I/vto" $label.logcat | grep -a -q "translate ms="; do
  s=$($ADB shell 'P=$(pidof dev.vto.overlay); echo "$(cat /proc/$P/cpuset 2>/dev/null) $(dumpsys power | grep -o "mWakefulness=[A-Za-z]*")"')
  echo "$(date +%T) $s" >> $label-cpuset.txt
  sleep 60
done
grep -a "I/vto" $label.logcat | grep -a "translate ms=" | tail -1
touch stop-$label
pwsh -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='adb.exe'\" | ? { \$_.CommandLine -match 'logcat' } | % { Stop-Process -Id \$_.ProcessId -Force }"
echo ready $(date +%T)
