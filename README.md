# Elefant Rumble

A private, on-device legal assistant. Document review, drafting, research, and translation — all running locally via Ollama. Your data never leaves your machine.

**End users:** See [Getting Started](agent_docs/2026-03-05-getting-started.md) for installation and usage.

---

## Architecture

```
┌──────────────────────────────────────────────┐
│  Tauri Shell (Rust)                          │
│  - window management, tray, IPC commands     │
│  - spawns sidecar, proxies frontend ↔ AI     │
├──────────────┬───────────────────────────────┤
│  Vue 3 UI    │  Python Sidecar (FastAPI)     │
│  Tailwind    │  PydanticAI + Ollama          │
│  Vue Router  │  SQLite (aiosqlite)           │
│  Pinia       │  SSE streaming                │
└──────────────┴───────────────────────────────┘
```

- **`src/`** — Vue 3 frontend (TypeScript, Tailwind, Pinia)
- **`src-tauri/src/`** — Rust backend (Tauri v2 commands, tray, sidecar management)
- **`src-tauri/sidecar/`** — Python FastAPI server (AI, chat, jobs, document processing)

## Developer Setup

### Prerequisites

- Node 20+ and [pnpm](https://pnpm.io/)
- Rust stable (via [rustup](https://rustup.rs/))
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

### Build Distributable

```bash
cd src-tauri/sidecar
uv run pyinstaller sidecar.spec --noconfirm
cp dist/rumble-sidecar ../binaries/rumble-sidecar-$(rustc -vV | grep host | cut -d' ' -f2)

cd ../..
pnpm tauri build
```

### Tests

```bash
pnpm vitest run                                    # frontend
cd src-tauri/sidecar && uv run pytest -q           # python
cd src-tauri && cargo test                         # rust
```

## License

Proprietary. All rights reserved.
