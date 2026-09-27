#!/bin/bash
# Co-op on one machine: a host and N clients, each WINDOWED on its own virtual display, connected
# over loopback on a random port.   Usage: coop.sh <seconds> <prefix> [extra args for EVERY instance]
# Defaults: --autodrop --botinput --autopick --netlog. Logs <prefix>-host.log / <prefix>-client.log
# (then -client2.log, -client3.log), screenshots <prefix>-host-<t>s.png / <prefix>-client-<t>s.png.
# Environment knobs (all optional; with none set this is the original two-instance run):
#   CLIENTS=1..3     how many clients join (default 1; 3 = a full squad of four)
#   JOIN_DELAY=6     seconds after the host starts before the first client joins (a drop-in test
#                    wants 30+); later clients follow 4 s apart
#   HOST_ARGS=...    extra args for the host only;  CLIENT_ARGS=... for the clients only
#   CLIENT_BOT=0     clients without --botinput (an idle player: a drop-in's autopilot drives)
SECS=$1; PFX=$2; shift 2
mkdir -p "$(dirname "$PFX")"
CLIENTS=${CLIENTS:-1}; JOIN_DELAY=${JOIN_DELAY:-6}; CLIENT_BOT=${CLIENT_BOT:-1}
PORT=$((5100 + RANDOM % 800))
free_display() { for n in $(seq 110 199); do [ -e /tmp/.X$n-lock ] || { echo $n; return; }; done; }
D=(); X=(); P=(); A=(); NAMES=(host)
for i in $(seq 1 "$CLIENTS"); do NAMES+=("client$([ "$i" = 1 ] || echo "$i")"); done
for i in $(seq 0 "$CLIENTS"); do
  d=$(free_display); Xvfb :$d -screen 0 1280x720x24 >/dev/null 2>&1 & X+=($!); D+=($d); sleep 1.5
  A+=("$(mktemp -d /tmp/astrobonk-${NAMES[$i]}-XXXX)")
done
BOT=--botinput; [ "$CLIENT_BOT" = 0 ] && BOT=
DISPLAY=:${D[0]} APPDATA=${A[0]} ./.astrobonk-bin --host --port $PORT --autodrop --botinput --autopick --netlog $HOST_ARGS "$@" > "$PFX-host.log" 2>&1 & P+=($!)
echo ":${D[0]}" > "$PFX-host.display"
sleep "$JOIN_DELAY"
for i in $(seq 1 "$CLIENTS"); do
  n=${NAMES[$i]}
  DISPLAY=:${D[$i]} APPDATA=${A[$i]} ./.astrobonk-bin --join 127.0.0.1 --port $PORT --autodrop $BOT --autopick --netlog $CLIENT_ARGS "$@" > "$PFX-$n.log" 2>&1 & P+=($!)
  echo ":${D[$i]}" > "$PFX-$n.display"
  [ "$i" -lt "$CLIENTS" ] && sleep 4
done
t=0
while [ $t -lt $SECS ]; do sleep 10; t=$((t+10))
  for i in $(seq 0 "$CLIENTS"); do DISPLAY=:${D[$i]} import -window root "$PFX-${NAMES[$i]}-${t}s.png" 2>/dev/null; done
  alive=1; for p in "${P[@]}"; do kill -0 $p 2>/dev/null || alive=0; done; [ $alive = 1 ] || break; done
kill "${P[@]}" 2>/dev/null; sleep 0.5; kill -9 "${P[@]}" 2>/dev/null; kill "${X[@]}" 2>/dev/null
for n in "${NAMES[@]}"; do
  echo "=== $n ($PFX-$n.log)"; grep -q panicked "$PFX-$n.log" && { echo "PANIC:"; grep -A12 panicked "$PFX-$n.log" | head -25; }
  grep -E "NET\[|NET (assigned|adopted|seated|peers)|NETENEMY|proxies|COOP" "$PFX-$n.log" | sed 's/\x1b\[[0-9;]*m//g' | tail -8
done
rm -rf "${A[@]}"
for n in "${NAMES[@]}"; do grep -q panicked "$PFX-$n.log" && exit 1; done
echo "RESULT: all $((CLIENTS + 1)) instances ran ${t}s without panic"
