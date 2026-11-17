#!/usr/bin/env bash
# ABOUTME: Smoke-tests a built macOS Rumble.app: both executables carry the library-validation
# ABOUTME: entitlement, and the bundled sidecar starts from the bundle and answers /health (#157).
set -euo pipefail

# CI's macos runner does not enforce library validation, so a sidecar that starts there can
# still fail on a user's Mac: the entitlement check is what catches #157 in CI.
ENTITLEMENT=com.apple.security.cs.disable-library-validation
EXECUTABLES=(elefant_rumble rumble-sidecar)
SMOKE_PORT=${RUMBLE_SMOKE_PORT:-18765}
HEALTH_WAIT_SECONDS=${RUMBLE_SMOKE_HEALTH_WAIT_SECONDS:-90}
HEALTH_REQUEST_SECONDS=2
STOP_WAIT_SECONDS=5

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <path to Elefant - Rumble.app>" >&2
  exit 2
fi
macos_dir="$1/Contents/MacOS"
data_dir=$(mktemp -d "${TMPDIR:-/tmp}/rumble-smoke.XXXXXX")
pid=

stop_sidecar() {
  if [ -n "$pid" ]; then
    kill "$pid" 2>/dev/null || true
    for _ in $(seq "$STOP_WAIT_SECONDS"); do
      kill -0 "$pid" 2>/dev/null || break
      sleep 1
    done
    kill -9 "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
  rm -rf "$data_dir"
}
trap stop_sidecar EXIT

# True, not merely present: an entitlement set to false still prints its key.
for name in "${EXECUTABLES[@]}"; do
  plist="$data_dir/$name.entitlements.plist"
  codesign -d --entitlements - --xml "$macos_dir/$name" >"$plist" 2>/dev/null || true
  if [ "$(/usr/libexec/PlistBuddy -c "Print :$ENTITLEMENT" "$plist" 2>/dev/null)" != "true" ]; then
    echo "$name lacks $ENTITLEMENT" >&2
    exit 1
  fi
done

log="$data_dir/sidecar.log"
secret=$(uuidgen)
RUMBLE_SIDECAR_SECRET="$secret" RUMBLE_DB_KEY=$(openssl rand -hex 32) \
  "$macos_dir/rumble-sidecar" --port "$SMOKE_PORT" --data-dir "$data_dir" >"$log" 2>&1 &
pid=$!

for _ in $(seq "$HEALTH_WAIT_SECONDS"); do
  if curl -sf --max-time "$HEALTH_REQUEST_SECONDS" -H "X-Rumble-Secret: $secret" "http://127.0.0.1:$SMOKE_PORT/health"; then
    echo
    echo "sidecar answered /health"
    exit 0
  fi
  if ! kill -0 "$pid" 2>/dev/null; then
    code=0
    wait "$pid" || code=$?
    echo "sidecar exited with code $code before /health answered" >&2
    cat "$log" >&2
    exit 1
  fi
  sleep 1
done
echo "sidecar did not answer /health within ${HEALTH_WAIT_SECONDS}s" >&2
cat "$log" >&2
exit 1
