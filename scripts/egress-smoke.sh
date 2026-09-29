#!/usr/bin/env bash
# ABOUTME: Egress smoke for a running packaged rumble: prints the deny-by-default pf procedure,
# ABOUTME: or samples the app's and Ollama's sockets (unprivileged) and reports non-loopback peers.
set -euo pipefail

DEFAULT_DURATION_SECONDS=60
SAMPLE_INTERVAL_SECONDS="${EGRESS_SAMPLE_INTERVAL_SECONDS:-1}"
OLLAMA_PROCESS_NAME="ollama"
PF_ANCHOR="com.apple/rumble-egress-smoke"
EXIT_PEERS_SEEN=1
EXIT_USAGE=2

usage() {
  cat <<EOF
usage: $(basename "$0") procedure
       $(basename "$0") observe <app-pid> [seconds]   (default ${DEFAULT_DURATION_SECONDS}s)

procedure  print the macOS pf steps that block all outbound traffic (needs sudo; not run for you)
observe    sample lsof for <app-pid>'s process tree and every '${OLLAMA_PROCESS_NAME}' process
           every ${SAMPLE_INTERVAL_SECONDS}s (EGRESS_SAMPLE_INTERVAL_SECONDS), then list non-loopback peers.
           Launches nothing. Exits ${EXIT_PEERS_SEEN} if any peer was seen.
EOF
  exit "$EXIT_USAGE"
}

print_procedure() {
  cat <<EOF
Deny-by-default egress check (macOS pf). Every command needs sudo; this script runs none of them.

0. Finish setup first while online: install Ollama, pull the model, launch rumble once.
   The claim under test is "offline after setup".

1. Block all outbound traffic except loopback (macOS's default pf.conf evaluates com.apple/*):
     printf 'pass out quick on lo0 all\nblock drop out log quick all\n' \\
       | sudo pfctl -a '${PF_ANCHOR}' -f -
     sudo pfctl -E          # note the Token it prints

2. Confirm the block holds (both must fail):
     curl -sS -m 5 https://example.com
     nc -z -G 5 1.1.1.1 443

3. Optional, and the best evidence: log what tried to leave.
     sudo ifconfig pflog0 create
     sudo tcpdump -n -e -ttt -i pflog0

4. Launch the packaged app. Exercise chat, draft and review end to end.
   Everything working = nothing needed the network. Anything in the tcpdump = something tried.
   (Optionally run '$(basename "$0") observe <app-pid>' alongside.)

5. Restore:
     sudo pfctl -a '${PF_ANCHOR}' -F all
     sudo pfctl -X <Token from step 1>
EOF
}

descendants() {
  local pid="$1" child
  echo "$pid"
  for child in $(pgrep -P "$pid" || true); do
    descendants "$child"
  done
}

watched_pids() {
  local app_pid="$1" root
  for root in "$app_pid" $(pgrep -ix "$OLLAMA_PROCESS_NAME" || true); do
    descendants "$root"
  done | sort -un | paste -sd, -
}

is_loopback() {
  case "$1" in
    127.*|\[::1\]|localhost) return 0 ;;
    *) return 1 ;;
  esac
}

# Prints "command(pid) -> peer" for each connected socket whose peer is not loopback.
sample_peers() {
  local pids="$1" line pid="" cmd="" peer
  lsof -nP -a -i -p "$pids" -F pcn 2>/dev/null | while IFS= read -r line; do
    case "$line" in
      p*) pid="${line#p}" ;;
      c*) cmd="${line#c}" ;;
      n*-\>*)
        peer="${line#*->}"
        is_loopback "${peer%:*}" || echo "${cmd}(${pid}) -> ${peer}"
        ;;
    esac
  done || true
}

observe() {
  local app_pid="$1" duration="$2" pids seen="" deadline samples=0
  ps -p "$app_pid" >/dev/null || { echo "no such process: $app_pid" >&2; exit "$EXIT_USAGE"; }

  deadline=$((SECONDS + duration))
  echo "Sampling every ${SAMPLE_INTERVAL_SECONDS}s for ${duration}s: pid ${app_pid} tree + '${OLLAMA_PROCESS_NAME}' processes"
  while (( SECONDS < deadline )); do
    pids="$(watched_pids "$app_pid")"
    seen+="$(sample_peers "$pids")"$'\n'
    samples=$((samples + 1))
    sleep "$SAMPLE_INTERVAL_SECONDS"
  done

  seen="$(printf '%s' "$seen" | sed '/^$/d' | sort -u)"
  echo "Watched (last sample): ${pids}"
  echo "Samples taken: ${samples}"
  if [[ -n "$seen" ]]; then
    echo "NON-LOOPBACK PEERS SEEN:"
    echo "$seen"
  else
    echo "No non-loopback peers in any sample."
  fi
  cat <<EOF

Limits: this samples; it does not watch. A connection opened and closed between samples is
missed, and UDP without a connected peer (e.g. DNS) is invisible. Unprivileged lsof sees only
your own processes (an Ollama running as another user, e.g. a Linux service, is not watched).
"None seen" is not proof. The deny-by-default block ('$(basename "$0") procedure') is.
EOF
  [[ -z "$seen" ]] || exit "$EXIT_PEERS_SEEN"
}

case "${1:-}" in
  procedure) print_procedure ;;
  observe)
    [[ -n "${2:-}" ]] || usage
    observe "$2" "${3:-$DEFAULT_DURATION_SECONDS}"
    ;;
  *) usage ;;
esac
