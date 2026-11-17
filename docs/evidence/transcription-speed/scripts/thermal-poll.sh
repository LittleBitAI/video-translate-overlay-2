#!/bin/sh
# Polls the phone's thermal status, temperatures and battery every 30 s into $1 until $2 exists.
export MSYS_NO_PATHCONV=1
ADB=${ADB:-adb}
while [ ! -e "$2" ]; do
  $ADB shell 'echo "t=$(date +%s) status=$(dumpsys thermalservice | sed -n "s/^Thermal Status: //p") $(dumpsys thermalservice | sed -n "/Current temperatures from HAL/,/Current cooling/p" | grep -oE "mValue=[0-9.]+, mType=[0-9], mName=(AP|SKIN|BAT)" | sed -E "s/mValue=([0-9.]+), mType=[0-9], mName=(.*)/\2=\1/" | tr "\n" " ") level=$(dumpsys battery | sed -n "s/^  level: //p")"' >> "$1"
  sleep 30
done
