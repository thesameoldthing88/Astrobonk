#!/bin/bash
# Build the checkout in the CURRENT DIRECTORY and put a debug-stripped binary at ./.astrobonk-bin.
# All worktrees share one target dir (the main checkout's), so dependencies compile once. A lock
# makes "build + copy" atomic across parallel worktrees. The root crate's fingerprint is stored
# RELATIVE to the package root and cannot tell two worktrees apart, so when the previous build
# came from a different directory we delete that fingerprint and force a recompile from THIS
# checkout. Errors print in short form; warnings are counted (pass -v to list them).
MAIN=$(cd "$(git rev-parse --git-common-dir)/.." && pwd)
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$MAIN/target}
exec 9>/tmp/astrobonk-build.lock
flock 9
LASTF=$CARGO_TARGET_DIR/.astrobonk-last-src
if [ "$(cat "$LASTF" 2>/dev/null)" != "$PWD" ]; then rm -rf "$CARGO_TARGET_DIR"/debug/.fingerprint/astrobonk-*; fi
LOG=$(mktemp)
cargo build --message-format=short > "$LOG" 2>&1
rc=$?
echo "$PWD" > "$LASTF"
if [ "$1" = "-v" ]; then grep -vE "^\s+(Compiling|Checking|Downloaded|Downloading)" "$LOG" | tail -n 150
else grep -vE "^\s+(Compiling|Checking|Downloaded|Downloading)" "$LOG" | grep -v ": warning: " | tail -n 100; fi
echo "(warnings: $(grep -c ': warning: ' "$LOG"))"
grep -q "wayland-client\|alsa\|libudev" "$LOG" && [ "$rc" != 0 ] && echo "HINT: system libraries missing — run tools/dev/setup.sh"
rm -f "$LOG"
if [ "$rc" = "0" ]; then objcopy --strip-debug "$CARGO_TARGET_DIR/debug/astrobonk" ./.astrobonk-bin.tmp && mv -f ./.astrobonk-bin.tmp ./.astrobonk-bin
  echo "BUILD OK -> ./.astrobonk-bin (built from $PWD; debug info stripped; panics still report file:line)"
else echo "BUILD FAILED (rc=$rc)"; fi
exit $rc
