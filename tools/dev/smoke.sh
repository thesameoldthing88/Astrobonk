#!/bin/bash
# The headless smoke matrix against ./.astrobonk-bin (build first with build.sh).
# Usage: smoke.sh [extra args appended to every run]. Every line must say PASS.
export APPDATA=$(mktemp -d /tmp/astrobonk-smoke-XXXX)
B=./.astrobonk-bin
[ -x "$B" ] || { echo "no ./.astrobonk-bin — run tools/dev/build.sh first"; exit 2; }
fail=0
for a in "--headless 2400" "--headless 1200 --fast-boss" "--headless 2400 --coop2" "--headless 1200 --fast-boss --planet mars" "--headless 1200 --fast-boss --planet darkmoon" "--headless 1200 --fast-boss --coop2"; do
  out=$(timeout 400 $B $a "$@" 2>&1); rc=$?
  sum=$(echo "$out" | grep -A1 "SMOKE SUMMARY" | tail -1)
  if echo "$out" | grep -q "SMOKE OK" && [ $rc = 0 ]; then echo "PASS  $a  | $sum"; else fail=1; echo "FAIL  $a (rc=$rc)"; echo "$out" | grep -iE "panic|error|SMOKE" | head -15; fi
done
rm -rf "$APPDATA"
exit $fail
