# Rumble — Installation Guide for Test Users

Rumble is a Tauri v2 desktop app for macOS, Windows, and Linux. The Python sidecar is bundled inside the app — test users only need Ollama.

---

## Test User Setup

### Prerequisites

Install Ollama — the local LLM runtime:

| Platform | Install |
|----------|---------|
| macOS | `brew install ollama` or download from https://ollama.com/download |
| Windows | Download installer from https://ollama.com/download |
| Linux | `curl -fsSL https://ollama.com/install.sh \| sh` |

Then pull the default model:
```bash
ollama pull llama3.2
```

### Install & Run

| Platform | Download | Install |
|----------|----------|---------|
| macOS (Apple Silicon) | `Elefant-Rumble-0.1.0-aarch64.dmg` | Open .dmg, drag to Applications |
| macOS (Intel) | `Elefant-Rumble-0.1.0-x86_64.dmg` | Open .dmg, drag to Applications |
| Windows | `Elefant-Rumble-0.1.0-x64-setup.exe` or `.msi` | Run installer |
| Linux (Debian/Ubuntu) | `elefant-rumble_0.1.0_amd64.deb` | `sudo dpkg -i <file>` |
| Linux (any) | `Elefant-Rumble_0.1.0_amd64.AppImage` | `chmod +x <file> && ./<file>` |

### Platform-specific notes

**macOS Gatekeeper:** Right-click → Open, then click Open in the dialog. Or run:
```bash
xattr -cr "/Applications/Elefant - Rumble.app"
```

**Windows Defender:** Click "More info" → "Run anyway" on the SmartScreen warning.

**Linux AppImage:** If the app doesn't launch, install FUSE:
```bash
sudo apt install libfuse2    # Ubuntu/Debian
sudo dnf install fuse-libs   # Fedora
```

### Verify It Works

- [ ] App window opens with "Elefant - Rumble"
- [ ] Tray icon appears in the menu bar / system tray
- [ ] Settings page shows mode = "Ollama"
- [ ] Document Draft → template selector shows different fields for Employment / NDA / Service Contract
- [ ] Chat → multi-turn conversation works (follow-up questions reference earlier messages)
- [ ] Research → "Start Research" button disables while processing

---

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| "sidecar not running" | Ollama must be running. macOS: open from Applications. Windows: check system tray. Linux: `ollama serve` |
| App won't open (macOS) | `xattr -cr "/Applications/Elefant - Rumble.app"` |
| App won't open (Windows) | Click "More info" → "Run anyway" |
| No models available | Pull a model: `ollama pull llama3.2` |
| Slow first response | Normal — Ollama loads the model into memory on first use (~30s) |
| App crashes on launch | Check system logs for `elefant_rumble` or `rumble-sidecar` errors |

---

## For Build Maintainers

### CI/CD — Automated Multi-Platform Builds

The repository includes a GitHub Actions workflow (`.github/workflows/build.yml`) that builds for all platforms automatically. Push a version tag to trigger:

```bash
git tag v0.1.0
git push origin v0.1.0
```

This produces:
| Platform | Artifact |
|----------|----------|
| macOS Apple Silicon | `.dmg` |
| macOS Intel | `.dmg` |
| Windows x64 | `.msi` + `.exe` (NSIS) |
| Linux x64 | `.deb` + `.AppImage` |

A draft GitHub Release is created automatically with all artifacts attached.

### Local Build from Source

#### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Node.js | 20+ | `brew install node` / `winget install OpenJS.NodeJS` / `sudo apt install nodejs` |
| pnpm | 9+ | `npm i -g pnpm` |
| Rust | stable | https://rustup.rs/ |
| Python | 3.12+ | `brew install python` / `winget install Python.Python.3.12` / `sudo apt install python3` |
| uv | latest | https://docs.astral.sh/uv/ |

**Linux only** — system libraries:
```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev \
  patchelf libxdo-dev libssl-dev
```

#### Build Steps

```bash
# 1. Install dependencies
pnpm install
cd src-tauri/sidecar && uv sync

# 2. Build the Python sidecar binary
uv run pyinstaller sidecar.spec --noconfirm

# 3. Copy to Tauri binaries dir with platform-specific name
# macOS Apple Silicon:
cp dist/rumble-sidecar ../binaries/rumble-sidecar-aarch64-apple-darwin
# macOS Intel:
cp dist/rumble-sidecar ../binaries/rumble-sidecar-x86_64-apple-darwin
# Linux:
cp dist/rumble-sidecar ../binaries/rumble-sidecar-x86_64-unknown-linux-gnu
# Windows (from cmd):
copy dist\rumble-sidecar.exe ..\binaries\rumble-sidecar-x86_64-pc-windows-msvc.exe

# 4. Build the Tauri app
cd ../..
# macOS:
pnpm tauri build --bundles dmg
# Windows:
pnpm tauri build --bundles msi,nsis
# Linux:
pnpm tauri build --bundles deb,appimage
```

Output locations:
- macOS: `src-tauri/target/release/bundle/dmg/`
- Windows: `src-tauri/target/release/bundle/msi/` and `nsis/`
- Linux: `src-tauri/target/release/bundle/deb/` and `appimage/`

### Run Tests Before Shipping

```bash
pnpm vitest run                            # 126 frontend unit tests
pnpm test:e2e                              # 72+ Playwright e2e tests
cd src-tauri/sidecar && uv run pytest -q   # 119 Python tests
cd src-tauri && cargo test                 # 37 Rust tests
```

---

## Architecture

```
Elefant - Rumble.app/           # macOS
Elefant - Rumble/               # Windows / Linux
└── Contents/
    ├── MacOS/ (or root on Windows/Linux)
    │   ├── elefant_rumble       # Rust/Tauri app
    │   └── rumble-sidecar       # Python sidecar binary
    ├── Resources/
    │   └── icon.icns / icon.ico
    └── Info.plist (macOS only)
```

- **Backend mode**: Ollama (default, local-first). BYOK and Premium modes available in Settings.
- **Sidecar**: Tauri spawns `rumble-sidecar` on a random port, health-checks, then proxies all API calls to it.
- **Data**: SQLite database stored in the platform-specific local app data directory (see SECURITY.md, "Where data lives"):
  - macOS: `~/Library/Application Support/com.ielegante.rumble/`
  - Windows: `%LOCALAPPDATA%/com.ielegante.rumble/` (moved from `%APPDATA%` on first launch of a build with #57)
  - Linux: `~/.local/share/com.ielegante.rumble/`
