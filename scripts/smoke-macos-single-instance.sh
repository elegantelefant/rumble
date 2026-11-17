#!/usr/bin/env bash
# ABOUTME: Launches a built Rumble.app twice and checks the second launch exits while the first
# ABOUTME: keeps running, so two instances never share one data dir and keychain key (#169).
set -euo pipefail

# Long enough for the first instance to claim the single-instance lock.
SETTLE_SECONDS=${RUMBLE_SMOKE_SETTLE_SECONDS:-15}
EXIT_WAIT_SECONDS=${RUMBLE_SMOKE_EXIT_WAIT_SECONDS:-20}

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <path to Elefant - Rumble.app>" >&2
  exit 2
fi
binary="$1/Contents/MacOS/elefant_rumble"

"$binary" >/dev/null 2>&1 &
first=$!
second=
trap 'kill "$first" ${second:+"$second"} 2>/dev/null || true' EXIT

sleep "$SETTLE_SECONDS"
if ! kill -0 "$first" 2>/dev/null; then
  echo "first instance exited within ${SETTLE_SECONDS}s" >&2
  exit 1
fi

"$binary" >/dev/null 2>&1 &
second=$!
for _ in $(seq "$EXIT_WAIT_SECONDS"); do
  kill -0 "$second" 2>/dev/null || break
  sleep 1
done
if kill -0 "$second" 2>/dev/null; then
  echo "second instance was still running after ${EXIT_WAIT_SECONDS}s" >&2
  exit 1
fi
code=0
wait "$second" || code=$?
if [ "$code" -ne 0 ]; then
  echo "second instance exited with code $code" >&2
  exit 1
fi
if ! kill -0 "$first" 2>/dev/null; then
  echo "first instance exited when the second launched" >&2
  exit 1
fi
echo "second launch exited; first instance still running"
