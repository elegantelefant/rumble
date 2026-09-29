# Elefant Rumble

A private, on-device legal assistant. Document review, drafting, research, and translation — all running locally via Ollama. Your data never leaves your machine.

Runs on **macOS** (Apple Silicon + Intel), **Windows**, and **Linux**.

**End users:** See [Getting Started](agent_docs/2026-03-05-getting-started.md) for installation and usage.
**Build maintainers:** See [Installation Guide](agent_docs/2026-03-05-installation-guide.md) for CI/CD and build-from-source.
**Security reviewers:** See [SECURITY.md](SECURITY.md) and [NETWORK.md](NETWORK.md).

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
# From the repo root:
pnpm install                          # frontend deps

# Sidecar deps, then build the sidecar binary. The binary is required before
# the first dev run — Tauri setup() spawns it directly and will not pick up a
# manually-run pnpm dev:sidecar process.
cd src-tauri/sidecar
uv sync
uv run pyinstaller sidecar.spec --noconfirm
mkdir -p ../binaries
cp dist/rumble-sidecar ../binaries/rumble-sidecar-$(rustc -vV | grep host | cut -d' ' -f2)
cd ../..

# Dev mode:
pnpm tauri dev                        # starts Tauri + Vite, and spawns the sidecar

# Optional, in a second terminal: run the sidecar from source instead, so Python
# changes take effect without rebuilding the binary.
pnpm dev:sidecar
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
mkdir -p ../binaries
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
