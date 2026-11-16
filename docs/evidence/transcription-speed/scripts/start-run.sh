#!/bin/sh
# Starts one in-app run of the lecture from the same app data each time:
# restores and verifies the backup, optionally installs an APK, stops T-money,
# waits for the phone's load to settle, deletes the lecture, opens it, presses
# detect and sends the app Home. Logcat and thermal polling run until stop-LABEL.
# usage: start-run.sh LABEL [APK relative to artifacts/tx-speed]
set -u
label=$1; apk=${2:-}
export MSYS_NO_PATHCONV=1
ADB=${ADB:-adb}
T=${T:?folder holding the run files}
CDP=cdp.mjs  # relative to $T: node and adb are Windows programs here
D=${D:-/tmp/vto-restorecheck}
# adb is a Windows program and gets no path conversion: give it paths relative to $T.
cd $T
n=$(pwsh -NoProfile -Command "(Get-Process | ? { \$_.ProcessName -match 'video-translate' } | Measure-Object).Count")
[ "$n" = 0 ] || { echo "Windows app is running"; exit 1; }
$ADB shell input keyevent KEYCODE_WAKEUP
$ADB shell am force-stop dev.vto.overlay
[ -n "$apk" ] && $ADB install -r "$apk" | tail -1
# Check the copy before deleting anything.
$ADB push app-data-backup.tar /data/local/tmp/vto-backup.tar | tail -1
want=$(sha256sum app-data-backup.tar | cut -d' ' -f1)
got=$($ADB shell sha256sum /data/local/tmp/vto-backup.tar | cut -d' ' -f1)
[ "$want" = "$got" ] || { echo "backup push failed"; exit 1; }
$ADB shell chmod 644 /data/local/tmp/vto-backup.tar
$ADB shell "run-as dev.vto.overlay sh -c 'rm -rf app.sqlite3 app.sqlite3-shm app.sqlite3-wal app_textures app_webview files no_backup shared_prefs sync audio/* && tar -xf /data/local/tmp/vto-backup.tar'"
$ADB shell rm /data/local/tmp/vto-backup.tar
rm -rf $D/b; mkdir -p $D/b
$ADB exec-out run-as dev.vto.overlay tar -cf - --exclude=./cache --exclude=./code_cache --exclude=./models --exclude=./audio . > $D/now.tar
tar -xf $D/now.tar -C $D/b
(cd $D/b && find . -type f -exec sha256sum {} + | sort -k2) > $D/b.txt
diff -q $D/a.txt $D/b.txt >/dev/null || { echo "restore differs"; exit 1; }
echo "restored IDENTICAL"
$ADB shell am force-stop com.lgt.tmoney
# Settle for a minute. The load average stays near 14 on this phone with the CPU
# idle (threads waiting on I/O), so it is recorded, not waited on.
sleep 60
echo "load $($ADB shell cat /proc/loadavg) mem $($ADB shell grep -E 'MemAvailable' /proc/meminfo)"
$ADB shell 'top -b -n1 -s 9 | sed -n 5,9p'
$ADB shell monkey -p dev.vto.overlay -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1
sleep 6
PID=$($ADB shell pidof dev.vto.overlay)
$ADB forward tcp:9333 localabstract:webview_devtools_remote_$PID >/dev/null
timeout 40 node $CDP 9333 - "(async()=>{const c=window.__TAURI_INTERNALS__.invoke; try{ await c('library_action',{action:{action:'delete_video',video_key:'zjkBMFhNj_g',title:'[1hr Talk] Intro to Large Language Models',confirmed:true},account_generation:1}); }catch(e){return {err:e}} const l=await c('get_library',{account_generation:1}); return (l.videos||[]).map(v=>v.video_key)})()"
timeout 30 node $CDP 9333 - "(async()=>{const c=window.__TAURI_INTERNALS__.invoke; try{ return await c('browser_open',{url:'https://m.youtube.com/watch?v=zjkBMFhNj_g',account_generation:1}); }catch(e){return {err:e}}})()"
sleep 8
timeout 20 node $CDP 9333 - "location.href"
$ADB logcat -c
($ADB logcat -v time > $T/$label.logcat 2>&1 &)
rm -f $T/stop-$label
(sh thermal-poll.sh $T/$label-thermal.txt $T/stop-$label >/dev/null 2>&1 &)
sleep 2
R=$(timeout 60 node $CDP 9333 - "$(cat $T/press-confirm.js)")
echo "$R" | tee $T/$label-press.json
$ADB shell input keyevent KEYCODE_HOME
date
