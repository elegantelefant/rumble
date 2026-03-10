# Rumble — Installation Guide for Test Users

Rumble is a Tauri v2 desktop app (macOS). The Python sidecar is bundled inside the `.app` — test users only need Ollama.

---

## Test User Setup (macOS Apple Silicon)

### Prerequisites

- [ ] **Install Ollama** — the local LLM runtime
  ```bash
  brew install ollama
  ```

### Install & Run

- [ ] **1. Download** `Elefant-Rumble-0.1.0-aarch64.zip` from the shared link
- [ ] **2. Unzip** and move `Elefant - Rumble.app` to `/Applications`
- [ ] **3. Start Ollama and pull the default model**
  ```bash
  ollama serve
  ollama pull llama3.2
  ```
- [ ] **4. Open the app** — double-click `Elefant - Rumble` in Applications
  - If macOS Gatekeeper blocks it: right-click → Open, then click Open in the dialog
  - Or run: `xattr -cr "/Applications/Elefant - Rumble.app"`

### Verify It Works

- [ ] App window opens with "Elefant - Rumble"
- [ ] Tray icon appears in the menu bar
- [ ] Settings page shows mode = "Ollama"
- [ ] Document Draft → template selector shows different fields for Employment / NDA / Service Contract
- [ ] Chat → multi-turn conversation works (follow-up questions reference earlier messages)
- [ ] Research → "Start Research" button disables while processing

---

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| "sidecar not running" | Ollama must be running first: `ollama serve` |
| App won't open (macOS block) | `xattr -cr "/Applications/Elefant - Rumble.app"` |
| No models available | Pull a model: `ollama pull llama3.2` |
| Slow first response | Normal — Ollama loads the model into memory on first use |
| App crashes on launch | Check Console.app for `elefant_rumble` or `rumble-sidecar` errors |

---

## For Build Maintainers

How to rebuild the distributable from source:

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Node.js | 18+ | `brew install node` |
| pnpm | 9+ | `npm i -g pnpm` |
| Rust | stable | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Python | 3.11+ | `brew install python` |
| uv | latest | `curl -LsSf https://astral.sh/uv/install.sh \| sh` |

### Build Steps

```bash
# 1. Clone and install dependencies
git clone <repo-url> && cd rumble
pnpm install
cd src-tauri/sidecar && uv sync

# 2. Build the Python sidecar binary (PyInstaller one-file mode)
uv run pyinstaller sidecar.spec --noconfirm
# Output: dist/rumble-sidecar (single 63MB binary)

# 3. Copy to Tauri binaries dir with platform-specific name
cp dist/rumble-sidecar ../binaries/rumble-sidecar-$(rustc --print host-tuple)

# 4. Build the Tauri .app bundle
cd ../..
pnpm tauri build --bundles app
# Output: src-tauri/target/release/bundle/macos/Elefant - Rumble.app

# 5. Zip for distribution
cd src-tauri/target/release/bundle/macos
zip -r "Elefant-Rumble-0.1.0-aarch64.zip" "Elefant - Rumble.app"
```

### Run Tests Before Shipping

```bash
pnpm vitest run                            # 123 frontend tests
cd src-tauri/sidecar && uv run pytest -q   # 113 Python tests
cd src-tauri && cargo test                  # 32 Rust tests
```

---

## Architecture

```
Elefant - Rumble.app/
└── Contents/
    ├── MacOS/
    │   ├── elefant_rumble       # Rust/Tauri app (14MB)
    │   └── rumble-sidecar       # Python sidecar binary (63MB)
    ├── Resources/
    │   └── icon.icns
    └── Info.plist
```

- **Backend mode**: Ollama (default, local-first). BYOK and Premium modes available in Settings.
- **Sidecar**: Tauri spawns `rumble-sidecar` on a random port, health-checks, then proxies all API calls to it.
- **Data**: SQLite database stored in `~/Library/Application Support/com.ielegante.rumble/`
