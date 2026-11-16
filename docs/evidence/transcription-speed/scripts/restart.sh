#!/bin/sh
# Kill-and-resume check of the writing stage, at no cost (stopped long before the glossary).
# Starts LABEL with APK1 (start-run.sh), force-stops the app once window KILL_AT is
# written, installs APK2 (if different), relaunches, sends it Home, and stops once
# window STOP_AT is written. The store is copied at the kill and at the end.
# usage: restart.sh LABEL APK1 APK2 KILL_AT STOP_AT   (APKs relative to artifacts/tx-speed)
set -u
label=$1; apk1=$2; apk2=$3; kill_at=$4; stop_at=$5
export MSYS_NO_PATHCONV=1
ADB=${ADB:-adb}
T=${T:?folder holding the run files}
cd $T
sh scripts/start-run.sh $label $apk1 || exit 1
last() { grep -a "I/vto" $label.logcat | grep -a -o "region i=[0-9]*" | tail -1 | cut -d= -f2; }
S=sync/${ACCOUNT:?the signed-in account store folder under sync/}  # the signed-in account's store
snap() { mkdir -p $label-$1; $ADB exec-out run-as dev.vto.overlay tar -cf - $S/sync.sqlite3 $S/sync.sqlite3-wal $S/sync.sqlite3-shm > $label-$1/db.tar; (cd $label-$1 && tar -xf db.tar && mv $S/* .); }
until [ "$(last)" -ge "$kill_at" ] 2>/dev/null; do sleep 2; done
$ADB shell am force-stop dev.vto.overlay
echo "killed after window $(last) $(date +%T)"
snap killed
[ "$apk2" != "$apk1" ] && $ADB install -r $apk2 | tail -1
$ADB shell monkey -p dev.vto.overlay -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1
sleep 5
$ADB shell input keyevent KEYCODE_HOME
echo "relaunched $(date +%T)"
until [ "$(last)" -ge "$stop_at" ] 2>/dev/null; do sleep 2; done
$ADB shell am force-stop dev.vto.overlay
echo "stopped after window $(last) $(date +%T)"
snap end
touch stop-$label
pwsh -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='adb.exe'\" | ? { \$_.CommandLine -match 'logcat' } | % { Stop-Process -Id \$_.ProcessId -Force }"
for s in killed end; do python -I scripts/cues-from-db.py $label-$s/sync.sqlite3 zjkBMFhNj_g $label-$s.json; done
