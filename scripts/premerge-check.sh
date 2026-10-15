#!/usr/bin/env bash
# ABOUTME: Runs the full local merge gate against a GitHub pull request merge ref.
# ABOUTME: Fetches an isolated merge worktree, reports four suite tallies, and cleans up on exit.

set -Eeuo pipefail

if [[ $# -ne 1 || ! $1 =~ ^[0-9]+$ ]]; then
  echo "usage: $0 <pull-request-number>" >&2
  exit 2
fi

pull_request=$1
repo_root=$(git rev-parse --show-toplevel)
temp_root=$(mktemp -d "${TMPDIR:-/tmp}/rumble-premerge.XXXXXX")
worktree="$temp_root/worktree"

cleanup() {
  status=$?
  git -C "$repo_root" worktree remove --force "$worktree" >/dev/null 2>&1 || true
  rm -rf "$temp_root"
  exit "$status"
}
trap cleanup EXIT

git -C "$repo_root" fetch origin "refs/pull/$pull_request/merge"
git -C "$repo_root" worktree add --detach "$worktree" FETCH_HEAD >/dev/null

run_suite() {
  local name=$1
  shift
  local log="$temp_root/$name.log"

  echo "== $name =="
  if ! (cd "$worktree" && "$@") >"$log" 2>&1; then
    cat "$log"
    echo "FAILED: $name" >&2
    exit 1
  fi
  cat "$log"
}

run_vitest() {
  pnpm install --frozen-lockfile
  pnpm build
  pnpm test
}

run_pytest() {
  (
    cd src-tauri/sidecar
    uv sync --locked
    uv run pytest -q
  )
}

run_cargo() {
  mkdir -p src-tauri/binaries
  touch "src-tauri/binaries/rumble-sidecar-$(rustc -vV | sed -n 's/^host: //p')"
  (cd src-tauri && cargo test)
}

run_drift() {
  pnpm generate:api
  pnpm generate:models
  git diff --exit-code -- src/api/models src-tauri/sidecar/models/generated.py
}

run_suite vitest run_vitest
run_suite pytest run_pytest
run_suite cargo run_cargo
run_suite drift run_drift

echo "TALLIES"
printf 'vitest: '
awk '/^      Tests / { print $2 " passed"; exit }' "$temp_root/vitest.log"
printf 'pytest: '
awk '/passed/ && /[0-9]+ passed/ { print $1 " passed"; exit }' "$temp_root/pytest.log"
printf 'cargo: '
awk '/test result:/ { total += $4 } END { print total " passed" }' "$temp_root/cargo.log"
echo 'drift: clean'
