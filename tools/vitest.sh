#!/usr/bin/env bash
set -euo pipefail

pnpm vitest run --maxWorkers=3 "$@"
