#!/bin/bash
# Run ./.astrobonk-bin WINDOWED on a private virtual display (software Vulkan, ~5-15 fps),
# screenshot every 8 s, and report panics.   Usage: win.sh <seconds> <shot_prefix> [game args...]
# Screenshots land at <shot_prefix>-<t>s.png. While it runs, drive input from another command with
#   DISPLAY=$(cat <shot_prefix>.display) xdotool key e
SECS=$1; PFX=$2; shift 2
mkdir -p "$(dirname "$PFX")"
for n in $(seq 110 199); do [ -e /tmp/.X$n-lock ] || { D=$n; break; }; done
Xvfb :$D -screen 0 1280x720x24 >/dev/null 2>&1 & XPID=$!
sleep 1.5
echo ":$D" > "$PFX.display"
export DISPLAY=:$D APPDATA=${APPDATA:-$(mktemp -d /tmp/astrobonk-win-XXXX)}
./.astrobonk-bin "$@" > "$PFX.log" 2>&1 & GPID=$!
t=0
while [ $t -lt $SECS ] && kill -0 $GPID 2>/dev/null; do sleep 8; t=$((t+8)); import -window root "$PFX-${t}s.png" 2>/dev/null; done
kill $GPID 2>/dev/null; sleep 0.5; kill -9 $GPID 2>/dev/null; kill $XPID 2>/dev/null
echo "--- log: $PFX.log ($(wc -l < "$PFX.log") lines); screenshots: $(ls $PFX-*s.png 2>/dev/null | tr '\n' ' ')"
grep -E "panicked|ERROR|B0001" "$PFX.log" | grep -v "error setting XSETTINGS" | head -20
grep -q "panicked" "$PFX.log" && { echo "RESULT: PANIC"; grep -A12 "panicked" "$PFX.log" | head -30; exit 1; }
echo "RESULT: ran ${t}s without panic"
