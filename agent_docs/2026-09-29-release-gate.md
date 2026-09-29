# Release gate

Owner directive, 2026-09-29. CI is not running, so this gate plus local suites are the only verification a change gets before merge.

## Scope

Every fix or closure PR. That includes docs PRs whose claims describe security, network or data-handling behaviour. Nothing merges until it is green on both workflows.

## Workflow 1: adversarial

- List the PR's material claims: what it says it fixes, closes, guarantees or documents.
- Give each material claim N independent opus refuters (usually 2 to 4 per PR, scaled to risk). Each refuter works alone and tries to prove the claim false.
- A refutation needs evidence: a `file:line` citation, a failing command with its output, or a reproduction. Opinion does not count.
- If a majority of refuters refute a claim, the claim is killed. A killed claim means the PR needs a fix.

## Workflow 2: dynamic release-readiness

- **Fresh-worktree suites.** Check out the PR head in a clean worktree and run the suites the change touches. Report exact pass/fail/skip counts, compared against `origin/main`.
  - Frontend: `pnpm vitest run` at the repo root.
  - Sidecar: `uv run pytest` in `src-tauri/sidecar`.
  - Tauri: `cargo test` in `src-tauri`. Needs `binaries/rumble-sidecar-aarch64-apple-darwin` copied in first.
- **Claims-vs-code audit.** Trace every user-facing claim in the diff to the code that implements it. That covers UI copy, `SECURITY.md`, `NETWORK.md`, README and PR-referenced docs. A claim with no backing code is a finding.
- **Packaged-build impact.** State what changes in the shipped app: bundled sidecar, Tauri config, capabilities, CSP and egress. If nothing changes, say "none".
- **Docs move with claims.** If a change alters a documented behaviour, the doc is updated in the same PR.
- **Docs or script PRs.** Run the script and confirm it does what the PR claims. Trace each doc claim to the code it cites.

## Actor model

- Sub-supervisors run both workflows and report a verdict for each PR: `release-ready` or `needs-fix`, listing only the findings that survived. They never merge.
- The top-level session decides and merges only when both workflows are green.

## Honesty rules

- Verify claims against the branch, never against the PR body.
- Every finding cites `file:line` or a command with its output.
- Report exact counts. "Tests pass" on its own is not a result.
- If something is unverified, say so. Do not infer it.
