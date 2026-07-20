# Elefant Rumble

A local-first legal assistant. Document review, drafting, research, and translation — running on-device via Ollama or your own provider keys, with an optional cloud tier (Premium) served by the Elefant API. In local modes, your documents stay on your machine.

Runs on **macOS** (Apple Silicon; Intel via Rosetta), **Windows**, and **Linux**.

**End users:** See [Getting Started](agent_docs/2026-03-05-getting-started.md) for installation and usage.
**Build maintainers:** See [Installation Guide](agent_docs/2026-03-05-installation-guide.md) for CI/CD and build-from-source.

---

## Architecture

```
┌──────────────────────────────────────────────┐
│  Tauri Shell (Rust)                          │
│  - window management, tray, IPC commands     │
│  - spawns sidecar, proxies frontend ↔ AI     │
│  - keychain tokens, cloud/sidecar routing    │
├──────────────┬───────────────────────────────┤
│  Vue 3 UI    │  Python Sidecar (FastAPI)     │
│  Tailwind    │  PydanticAI + Ollama          │
│  Vue Router  │  SQLite (aiosqlite)           │
│  vue-query   │  SSE streaming                │
└──────────────┴───────────────────────────────┘
        │
        ▼  /api/v1/* paths (everything in Premium mode)
   Elefant Cloud API (api.elefant.com)
```

- **`src/`** — Vue 3 frontend (TypeScript, Tailwind, TanStack vue-query)
- **`src-tauri/src/`** — Rust backend (Tauri v2 commands, tray, sidecar management, API proxy)
- **`src-tauri/sidecar/`** — Python FastAPI server (AI, chat, jobs, document processing)

### Backend modes & routing

All frontend API calls go through the Rust `api_call` IPC command (no direct `fetch`, except SSE streaming which connects straight to the sidecar). Rust picks the target per mode:

- **Ollama** / **BYOK** — paths under `/api/v1/` go to the cloud API; everything else goes to the local sidecar.
- **Premium** — everything goes to the cloud API, authenticated with a keychain-stored bearer token.

### API contract

`openapi.json` at the repo root (vendored from the backend spec, currently 0.4.0) is the source of truth for API types. After updating it, regenerate both clients:

```bash
pnpm generate:api      # orval → src/api/generated + src/api/models (TypeScript)
pnpm generate:models   # datamodel-codegen → src-tauri/sidecar/models/generated.py (Pydantic)
```

Never hand-edit generated files — the contract-drift CI gate regenerates and fails on any diff.

## Developer Setup

### Prerequisites

- Node 20+ and [pnpm](https://pnpm.io/)
- Rust 1.90.0 via [rustup](https://rustup.rs/) (pinned in `rust-toolchain.toml`, picked up automatically)
- Python 3.12+ and [uv](https://docs.astral.sh/uv/)
- [Ollama](https://ollama.com/) running locally

### Install & Run

```bash
pnpm install                          # frontend deps
cd src-tauri/sidecar && uv sync       # sidecar deps

# Dev mode (two terminals):
pnpm dev:sidecar                      # start Python sidecar
pnpm tauri dev                        # start Tauri + Vite
```

### Linux System Dependencies

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev \
  patchelf libxdo-dev libssl-dev
```

### Build Distributable

```bash
cd src-tauri/sidecar
uv run pyinstaller sidecar.spec --noconfirm
cp dist/rumble-sidecar ../binaries/rumble-sidecar-$(rustc -vV | grep host | cut -d' ' -f2)

cd ../..
pnpm tauri build
```

Or push a version tag to build all platforms via GitHub Actions:

```bash
git tag v0.1.0 && git push origin v0.1.0
```

### Tests

```bash
pnpm vitest run                                    # frontend (126 tests)
pnpm test:e2e                                     # playwright e2e (72+ tests)
cd src-tauri/sidecar && uv run pytest -q           # python (119 tests)
cd src-tauri && cargo test                         # rust (37 tests)
```

## License

Proprietary. All rights reserved.
