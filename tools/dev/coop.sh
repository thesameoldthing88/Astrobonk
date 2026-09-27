#!/bin/bash
# Two-instance co-op: a host and a client, each WINDOWED on its own virtual display, connected over
# loopback on a random port.   Usage: coop.sh <seconds> <prefix> [extra args for BOTH instances]
# Defaults: --autodrop --botinput --autopick --netlog. Logs <prefix>-host.log / <prefix>-client.log,
# screenshots <prefix>-host-<t>s.png / <prefix>-client-<t>s.png.
SECS=$1; PFX=$2; shift 2
mkdir -p "$(dirname "$PFX")"
PORT=$((5100 + RANDOM % 800))
for n in $(seq 110 199); do [ -e /tmp/.X$n-lock ] || { D1=$n; break; }; done
Xvfb :$D1 -screen 0 1280x720x24 >/dev/null 2>&1 & X1=$!; sleep 1
for n in $(seq 110 199); do [ -e /tmp/.X$n-lock ] || { D2=$n; break; }; done
Xvfb :$D2 -screen 0 1280x720x24 >/dev/null 2>&1 & X2=$!; sleep 1.5
A1=$(mktemp -d /tmp/astrobonk-host-XXXX); A2=$(mktemp -d /tmp/astrobonk-client-XXXX)
DISPLAY=:$D1 APPDATA=$A1 ./.astrobonk-bin --host --port $PORT --autodrop --botinput --autopick --netlog "$@" > "$PFX-host.log" 2>&1 & H=$!
sleep 6
DISPLAY=:$D2 APPDATA=$A2 ./.astrobonk-bin --join 127.0.0.1 --port $PORT --autodrop --botinput --autopick --netlog "$@" > "$PFX-client.log" 2>&1 & C=$!
echo ":$D1" > "$PFX-host.display"; echo ":$D2" > "$PFX-client.display"
t=0
while [ $t -lt $SECS ]; do sleep 10; t=$((t+10))
  DISPLAY=:$D1 import -window root "$PFX-host-${t}s.png" 2>/dev/null; DISPLAY=:$D2 import -window root "$PFX-client-${t}s.png" 2>/dev/null
  kill -0 $H 2>/dev/null || break; kill -0 $C 2>/dev/null || break; done
kill $H $C 2>/dev/null; sleep 0.5; kill -9 $H $C 2>/dev/null; kill $X1 $X2 2>/dev/null
for r in host client; do
  echo "=== $r ($PFX-$r.log)"; grep -q panicked "$PFX-$r.log" && { echo "PANIC:"; grep -A12 panicked "$PFX-$r.log" | head -25; }
  grep -E "NET\[|NET (assigned|adopted|seated|peers)|NETENEMY|proxies" "$PFX-$r.log" | sed 's/\x1b\[[0-9;]*m//g' | tail -8
done
rm -rf "$A1" "$A2"
grep -q panicked "$PFX-host.log" "$PFX-client.log" && exit 1; echo "RESULT: both instances ran ${t}s without panic"
