# Contributing to Elefant Rumble

Thanks for helping. This file is the short version; `CLAUDE.md` and `docs/agents/working-with-the-fleet.md` hold the full process.

## Run it locally

Prerequisites: Node 20+ with pnpm 9, Rust stable, Python 3.12+ with [uv](https://docs.astral.sh/uv/), and [Ollama](https://ollama.com/) running.

```sh
pnpm install
(cd src-tauri/sidecar && uv sync)
pnpm tauri dev          # needs the sidecar binary built once; see README "Install & Run"
pnpm dev:sidecar        # optional: sidecar from source on :11435
```

## Run the suites

```sh
pnpm build                                        # vue-tsc typecheck + vite build
pnpm test                                         # vitest
pnpm test:e2e                                     # playwright (UI changes)
(cd src-tauri/sidecar && uv run pytest -q)        # sidecar
mkdir -p src-tauri/binaries && touch src-tauri/binaries/rumble-sidecar-$(rustc -vV | sed -n 's/^host: //p')
(cd src-tauri && cargo test)                      # Rust host (stub binary above)
pnpm generate:api && pnpm generate:models && git diff --exit-code -- src/api/models src-tauri/sidecar/models/generated.py   # contract drift
```

CI is intentionally local-first: the GitHub Actions workflows exist but are not the merge gate. Run the suites your change touches on your own machine and quote the exact pass counts in the PR.

## Pull requests

- Open or link an issue first; put `Refs #N` in the PR body.
- One concern per PR. [Conventional commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, `chore:` ...).
- Tests are required for behaviour changes: behaviour-named, one concept each, clean output.
- Every source file (`.ts`, `.vue`, `.rs`, `.py`) opens with two `ABOUTME:` comment lines.
- No secrets in code, config or fixtures.
- Every PR goes through an adversarial review gate before merge: independent reviewers try to refute each claim the PR makes, and the suites are re-run on a fresh checkout. See [`agent_docs/2026-09-29-release-gate.md`](agent_docs/2026-09-29-release-gate.md).

Security issues: see [`SECURITY.md`](SECURITY.md).

By contributing you agree your contribution is licensed under the [Apache License 2.0](LICENSE).
