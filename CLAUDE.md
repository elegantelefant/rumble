# CLAUDE.md

Repo-level instructions for humans and AI coding agents working on Elefant Rumble (`elegantelefant/rumble`; the laptop checkout is named `ivory`). The owner's global `~/.claude/CLAUDE.md` applies everywhere; this file holds only what you learn by working here. The process in full: `docs/agents/working-with-the-fleet.md`.

## What this is

A local-first legal-AI desktop app on Tauri v2: a Vue 3 frontend in a webview, a Rust host process, a bundled Python FastAPI sidecar running local AI (Ollama or the user's own provider key), and a Premium tier that calls the Elefant cloud API. Three languages, four test suites, one contract decision (below). Narrative onboarding lives on the closed PR #1 branch (`agent_docs/2026-07-20-intern-intro.md`) — bringing it to `main` is a follow-up; `agent_docs/2026-04-03-readiness-session-notes.md` is the current reality, `README.md` on `main` still tells the local-only story.

| Path | What |
|---|---|
| `src/` | Vue 3 + TS + Tailwind 3 + vue-router + TanStack vue-query. `views/`, IPC mutator `api/client.ts`, hand-written sidecar client `api/sidecar.ts` (snake_case), generated types `api/models/` |
| `src-tauri/src/lib.rs` | The whole Rust host (~1.1k lines): `api_call`, `resolve_url`, keychain, sidecar spawn (`PORT:{port}` handshake), tray, export commands. The Rust tests live here |
| `src-tauri/capabilities/default.json` | The webview→core permission surface — the file people audit at open-source time |
| `src-tauri/sidecar/` | FastAPI + PydanticAI + aiosqlite: `routes/{ai,chat,health,jobs}.py`, `services/{db,jobs,llm,prompts}.py`, `models/generated.py` (generated), `tests/` (pytest), `sidecar.spec` (PyInstaller) |
| `sidecar-openapi.json` | The sidecar's **own frozen spec** (0.1.0 lineage). Decision 2026-09-03 (#33, #39): it does not track birepo |
| `openapi.json` | The vendored elefant **cloud contract** (0.305.0 lineage, subscription-gated API) for premium call sites; synced deliberately from the monorepo |
| `tests/` | Vitest (happy-dom) unit + component tests; `test-utils.ts` |
| `e2e/`, `selenium-tests/` | Playwright against Vite on `:1420`; Selenium (mocha) against `vite preview` in CI |
| `.github/workflows/` | `test.yml` (vitest, pytest, cargo test), `contract-drift.yml`, `webdriver.yml`, `build.yml` (tag `v*` → dmg/msi/deb/AppImage draft release) |
| `agent_docs/` | Date-versioned; `2026-09-02-contract-0207-migration-scoping.md` holds the contract decision |

Routing in one line: the frontend never `fetch`es; HTTP calls go through `invoke('api_call')` (except `extract_document` and chat streaming's `stream_message`, which have their own host commands, mode-guarded the same way) and `resolve_url` sends it to the sidecar on `127.0.0.1:{port}` or refuses it ("requires Elefant Premium": `/api/v1/*` in local/BYOK, everything in Premium until the L0 payload contract) — it never returns a cloud URL. Cloud auth is a keychain bearer, attached only to non-sidecar URLs (of which there are none today); the sidecar's shared secret arrives by env (`RUMBLE_SIDECAR_SECRET`), never argv (#8).

## Commands (from `package.json`, `pyproject.toml`, CI; nothing else exists)

```sh
pnpm install                                     # pnpm 9 / Node 20 in CI
cd src-tauri/sidecar && uv sync                  # Python 3.12
pnpm tauri dev                                   # the app; spawns the sidecar BINARY — build it once (README)
pnpm dev:sidecar                                 # sidecar from source on :11435
pnpm build                                       # vue-tsc --noEmit && vite build — the TS typecheck
pnpm test                                        # vitest run → tests/**, src/**
pnpm test:e2e                                    # playwright; starts vite on :1420
cd src-tauri/sidecar && uv run pytest -q         # sidecar suite
mkdir -p src-tauri/binaries && touch src-tauri/binaries/rumble-sidecar-$(rustc -vV | sed -n 's/^host: //p')   # stub so build.rs passes, as CI does
cd src-tauri && cargo test                       # Rust host
pnpm generate:api && pnpm generate:models && git diff --exit-code -- src/api/models src-tauri/sidecar/models/generated.py   # the drift gate
pnpm tauri build                                 # packaged app; needs the real PyInstaller sidecar (README)
```

No lint or format gate exists in any of the three languages: no ESLint/Prettier, no ruff config, `cargo fmt --check` / `cargo clippy` not run in CI. Run them locally; do not make a verdict depend on them until CI does. CI installs Rust `stable`; the `rust-toolchain.toml` pin (1.90.0) exists only on the closed PR #1 branch — pinning on `main` is a follow-up.

## Merge gate

GitHub Actions is billing-gated and currently runs zero steps in every job. It is never the blocker and never the evidence. The gate is the full local suite on the exact merged tree:

```sh
git fetch origin refs/pull/<N>/merge && git checkout -q FETCH_HEAD
pnpm install && pnpm build && pnpm test
(cd src-tauri/sidecar && uv sync && uv run pytest -q)
(cd src-tauri && cargo test)                    # with the stub binary above
pnpm generate:api && pnpm generate:models && git diff --exit-code -- src/api/models src-tauri/sidecar/models/generated.py
```

All four tallies quoted in the verdict (`vitest 141 passed`, `pytest 146 passed`, `cargo 37 passed`, drift clean). What the ARBITER RULE adds by touched area:

| Touched | Also run |
|---|---|
| `src/views/*`, components | `pnpm test:e2e` |
| `src/api/sidecar.ts`, `sidecar/routes/*` | the pytest route tests **and** the vitest tests that mock that call — both sides of the wire |
| `lib.rs` spawn / `resolve_url` / keychain, `capabilities/*.json`, `tauri.conf.json`, `sidecar.spec` | a packaged build (`pnpm tauri build`) smoked by a human — #35 (BYOK unreachable in the packaged app) is invisible to every automated suite |
| `sidecar-openapi.json` | it is frozen; changing it is a contract decision — ask first (#34 / #42) |

Not yet in place: a `scripts/premerge-check.sh` that runs the block above on the merge ref; lint/format gates; the toolchain pin on `main`. Until then the verifier runs the commands by hand and says so.

## Issues, branches, PRs, verdicts

- Work is a GitHub issue in this repo (`gh issue …`). Linear mirrors them as `ELE-nnn`; the GitHub issue is the record agents read and write.
- Owner rulings live in the issue body under `## OWNER DECISION <date>`, verbatim. They are not re-litigated. (#39 is the model: purpose settled contract ownership; the consequences are listed under it.)
- The five triage labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`) do not exist here yet — only GitHub's defaults. Creating them is the first follow-up.
- Branch `codex/<issue>-<slug>` for lanes; humans may keep `feat/`, `fix/` but the issue number is in the branch or the first body line. PR to `main`. `Refs #N`, never `Fixes`/`Closes` — merged ≠ released ≠ verified.
- PR body: tick-box progress, WIRE PROOF (the named test that proves the change through the production path — `invoke('api_call')` → `resolve_url` → the route, or the sidecar route through its FastAPI test client — with counts), a mutation ledger for every test added, local evidence at the pushed head SHA. Template in `docs/agents/working-with-the-fleet.md`.
- Every PR gets one independent adversarial verification comment ending `Verdict: MERGE @<head sha>` or `Verdict: HOLD @<head sha>: …`. A self-posted "recommend merge" is a self-check, not a verdict. Merge only on MERGE.
- ARBITER RULE, TWO-WRITERS RULE, closure on a packaged build: `docs/agents/working-with-the-fleet.md`.

## Increments and naming

- Ship the smallest slice that leaves the repo better; follow-ups are follow-ups — file the issue, link it, do not fold it in (#24 died carrying two changes; #17 grew at review).
- UI increments match the existing look: the Tailwind 3 tokens and components already in `src/components/`, lucide icons, the toast provider. No new design system, no new dependency for a UI slice; a new Rust crate is a `default-features = false` question first (#10: `docx-rs`).
- A UIUX issue closes only with a screenshot from a packaged build.
- Every source file (`.ts`, `.vue`, `.rs`, `.py`) opens with two `ABOUTME:` lines. Names say what, never how or when.
- Conventional commits, one concern each; `git commit -- <paths>`, never `git add .`.

## Who does what

| | Humans (interns, devs) | Fleet (Codex/Claude lanes, Claude supervising) |
|---|---|---|
| Picks | `ready-for-human`, anything assigned, anything found while reading | `ready-for-agent`, priority order, one lane per issue |
| Files issues | yes — what you find while reading is an issue first, a PR second (#18–#23 are the model) | yes — verifier findings become issues |
| Opens PRs | same shape as the fleet's; run the gate locally before asking for review | same |
| Verifies | may run Claude/Codex review locally and post the notes; the verdict comes from the fleet | posts the verdict on every PR, human or agent |
| Pushes to a branch | only their own | only their own — never a human's |
| Runs the packaged app, takes screenshots | yes — the part only a human can do | no |
| Merges, tags releases | no | supervisor merges on MERGE; a `v*` tag (a public draft release) stays owner-gated |

## Asking the owner (owner-gated protocol)

Owner-gated = only the owner can do it: money, prod / credentials / signing, release tags, privacy claims in the UI (#19, #20), product and design taste, contract shape (#34: `/extract` vs `/review/upload`, multipart vs base64), what Premium sells. Everything else: decide, state the decision in the PR, ship; a reviewer can overturn.

When you do need him: one comment on the issue under `## OWNER QUESTIONS <date>`, numbered, each with the question, your recommendation, what it blocks, and the default you will take if there is no answer by a stated date. Batch — one such comment per issue per day. Slack carries a one-line pointer, never the question. Whoever receives the answer pastes it verbatim into the issue body under `## OWNER DECISION <date>` in the same turn. Never re-ask a decided question; never "raise it with Ian" out of band and leave the issue silent (#17). A draft PR whose body says "two open questions on #34" should have those two questions, with recommendations and defaults, on #34 (#42).

## Standing rules that bite in this stack

- Tests: behaviour-named, one concept each, pristine output — an unmocked `listModels` on mount printing `console.error` in two view tests is a defect (#26). Assert the visible effect (the toast copy), not that `invoke` wasn't called (#10). Never mock the code under test; mock at the boundary (`invoke`, `fetch`, keychain). Standards: `agent_docs/2026-03-03-vitest-testing-standards.md`.
- Tauri `invoke` rejects with a **string**, not an `Error`: `error instanceof Error` is never true. Use `typeof error === "string" ? error : String(error)` and make test mocks reject with strings (#10). `DocumentDraftView`, `ResearchView`, `TranslationView` still carry the old pattern — agreed follow-up.
- Cancel is not success: a native-dialog command returns `Result<bool, String>`; toast success only on `true` (#10).
- No blob + `<a download>`: it silently drops in WKWebView. Export through a Rust command with the native save dialog (#24 → #10).
- `capabilities/default.json` grants only what the webview actually imports; a dialog opened from Rust needs no `dialog:*` entry (#10).
- Nothing "hardcoded for now" ships: `SilverEcho951`, `sync.elefantapp.com`, seeded secrets, fake model lists, encryption claims the code does not honour (#17, #26, #19, #20). Blank defaults, real data, or nothing.
- Secrets env-only: provider keys in the OS keychain via Rust, the sidecar secret by env; never in `tauri.conf.json`, never in a test fixture, never in chat.
- Legal facts (citations, section numbers, dates, in-force status) come from the corpus through the cloud API, never from the local model. Local modes must say so in copy rather than imply authority; never let a prompt in `services/prompts.py` invent authorities.
- Async I/O, sync CPU: sidecar routes are `async`; `blocking_save_file` inside an async Tauri command parks a worker — fine at this scale, `spawn_blocking` if it matters (#10 caveat).
- Machine: one shared laptop. Work in your own clone or worktree under the session scratchpad — never in `/Users/ianc/multilang/elefant/ivory` (it sits on the closed PR #1 branch, 17 ahead / 19 behind `main`). `vitest --maxWorkers=3`, `playwright --workers=1`, one `cargo test` at a time.

## Landmines

- `.claude/` is gitignored here: skills and workflows are not versioned in this repo. The process is this file plus `docs/agents/`.
- `router.ts` hardcodes `isAuthenticated = true`; `src/modules/backend/backendClient.ts` still has `mock*` functions — check which path a view actually uses before assuming.
- Bundle id is `com.ielegante.rumble`, which decides the DB path (`app_local_data_dir`; a DB in the older Windows roaming `app_data_dir` is moved on startup, #57). The sidecar purges jobs older than 30 days at startup, so a test DB with old jobs shrinks on launch. The sidecar binary is gitignored; `pnpm tauri dev` needs a built one, `pnpm dev:sidecar` alone is not enough.
- `pnpm install` can be refused by pnpm's minimum-release-age policy on a freshly published transitive dep (#36 comment) — wait; never bypass.
- The sidecar takes its mode only from `RUMBLE_BACKEND_MODE`, failing closed to `ollama` (a `BYOK_API_KEY` alone never selects BYOK). The host sets it at spawn and respawns the sidecar on every mode switch (`set_backend_mode`), with `BYOK_API_KEY` blanked outside byok. `pnpm dev:sidecar` sets neither, so a source sidecar runs `ollama` unless you export them. The host mode is not persisted: every launch starts in `ollama`.
- In ollama mode `llm.py` refuses a requested model that isn't a pulled local model in `/api/tags`, and any cloud one (by `remote_host`/`remote_model`, then name suffix); with no model requested it uses `OLLAMA_DEFAULT_MODEL` (refused, never skipped, if that is a cloud model), else the first pulled local model (#26). This resolution and cloud check run on every request, uncached, because `ollama cp` can make a local name remote at runtime; `/ready` is `not_ready` when it finds no usable local model.
