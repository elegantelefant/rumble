#!/usr/bin/env bash
# ABOUTME: Smoke-tests a built macOS Rumble.app: library validation is never disabled, the app has the
# ABOUTME: hardened runtime, and the bundled sidecar helper starts from the bundle and answers /health (#157).
set -euo pipefail

DISABLE_LIBRARY_VALIDATION=com.apple.security.cs.disable-library-validation
# The other hardened-runtime exceptions; each would let code into the app that holds the keychain items.
RUNTIME_EXCEPTIONS=(
  com.apple.security.cs.allow-dyld-environment-variables
  com.apple.security.cs.allow-unsigned-executable-memory
  com.apple.security.cs.disable-executable-page-protection
  com.apple.security.cs.allow-jit
  com.apple.security.get-task-allow
)
APP_BINARY=MacOS/elefant_rumble
HELPER=Helpers/rumble-sidecar.app/Contents/MacOS/rumble-sidecar
HEALTH_WAIT_SECONDS=${RUMBLE_SMOKE_HEALTH_WAIT_SECONDS:-90}
HEALTH_REQUEST_SECONDS=2
STOP_WAIT_SECONDS=5

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <path to Elefant - Rumble.app>" >&2
  exit 2
fi
contents="$1/Contents"
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

fail() {
  echo "$1" >&2
  exit 1
}

# codesign's signature summary for one executable: its flags line, its entitlements as XML.
flags_of() { codesign -dv "$contents/$1" 2>&1 | sed -n 's/.*flags=0x[0-9a-f]*(\([^)]*\)).*/\1/p'; }
# Whether executable $1 is signed with entitlement $2 set to true.
entitled() {
  local plist="$data_dir/entitlements.plist"
  codesign -d --entitlements - --xml "$contents/$1" >"$plist" 2>/dev/null || true
  [ "$(/usr/libexec/PlistBuddy -c "Print :$2" "$plist" 2>/dev/null)" = "true" ]
}

for executable in "$APP_BINARY" "$HELPER"; do
  if entitled "$executable" "$DISABLE_LIBRARY_VALIDATION"; then
    fail "${executable##*/} disables library validation"
  fi
done
for exception in "${RUNTIME_EXCEPTIONS[@]}"; do
  if entitled "$APP_BINARY" "$exception"; then
    fail "elefant_rumble has the hardened-runtime exception $exception"
  fi
done
# The app holds the keychain items: the hardened runtime keeps library validation on for it.
[[ ",$(flags_of "$APP_BINARY")," == *,runtime,* ]] || fail "elefant_rumble lacks the hardened runtime"
# An ad-hoc signature has no Team ID, so library validation would refuse every library the helper
# loads; the CI runner doesn't enforce it, a user's Mac does. Developer ID signing (#102) lifts this.
helper_flags=",$(flags_of "$HELPER"),"
if [[ $helper_flags == *,adhoc,* && $helper_flags == *,runtime,* ]]; then
  fail "ad-hoc rumble-sidecar has the hardened runtime: library validation will refuse its libraries"
fi

# A port nothing is listening on now, so a tunnel or another app on a fixed one can't mask the result.
SMOKE_PORT=${RUMBLE_SMOKE_PORT:-$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')}
log="$data_dir/sidecar.log"
secret=$(uuidgen)
RUMBLE_SIDECAR_SECRET="$secret" RUMBLE_DB_KEY=$(openssl rand -hex 32) \
  "$contents/$HELPER" --port "$SMOKE_PORT" --data-dir "$data_dir" >"$log" 2>&1 &
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
    pid=
    echo "sidecar exited with code $code before /health answered" >&2
    cat "$log" >&2
    exit 1
  fi
  sleep 1
done
echo "sidecar did not answer /health within ${HEALTH_WAIT_SECONDS}s" >&2
cat "$log" >&2
exit 1
