#!/usr/bin/env sh
# Manual capture of the current screen, without a11y-report (see the spec).
# Usage: scripts/capture-android.sh <screen_name>   (run from a run folder)
# Prefer `a11y-report capture <screen_name>`, which also updates run.json.
set -eu
name="${1:?usage: capture-android.sh <screen_name>}"
mkdir -p screens
adb shell uiautomator dump /sdcard/window.xml && adb pull /sdcard/window.xml "screens/$name.xml"
adb exec-out screencap -p > "screens/$name.png"
echo "captured screens/$name.xml and screens/$name.png — add the screen to run.json"
