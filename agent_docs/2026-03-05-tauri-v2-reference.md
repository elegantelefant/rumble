# Tauri v2 Reference — Writing to Distributing

Compiled from https://v2.tauri.app/ — covers core concepts, development patterns, security, distribution, and practical recipes.

---

## Table of Contents

1. [Architecture & Process Model](#1-architecture--process-model)
2. [Project Structure](#2-project-structure)
3. [Configuration](#3-configuration)
4. [IPC — Commands & Events](#4-ipc--commands--events)
5. [State Management](#5-state-management)
6. [Sidecars (External Binaries)](#6-sidecars-external-binaries)
7. [Security & Capabilities](#7-security--capabilities)
8. [System Tray](#8-system-tray)
9. [Window Customization](#9-window-customization)
10. [Plugins](#10-plugins)
11. [App Icons](#11-app-icons)
12. [Building & Bundle Formats](#12-building--bundle-formats)
13. [Code Signing](#13-code-signing)
14. [Auto-Updater](#14-auto-updater)
15. [CI/CD (GitHub Actions)](#15-cicd-github-actions)
16. [App Size Optimization](#16-app-size-optimization)
17. [Debugging](#17-debugging)
18. [Common Pitfalls](#18-common-pitfalls)

---

## 1. Architecture & Process Model

Tauri apps have two processes:

| Process | Runtime | Access | Role |
|---------|---------|--------|------|
| **Core** (main) | Rust | Full system | Spawn windows, IPC, system APIs |
| **WebView** (renderer) | OS WebView | Sandboxed | UI rendering via HTML/CSS/JS |

- The Core process is the Rust binary — it owns the event loop, manages windows, and runs commands.
- The WebView uses the **OS-native** WebView (WebKit on macOS, WebView2 on Windows, WebKitGTK on Linux) — no bundled Chromium.
- Communication is through **IPC** (commands and events). The WebView never has direct system access.

### Key Implication
All system operations (file I/O, shell, network, database) happen in Rust. The frontend requests them via `invoke()`.

---

## 2. Project Structure

```
my-app/
├── src/                    # Frontend source (Vue, React, etc.)
├── src-tauri/
│   ├── Cargo.toml          # Rust dependencies
│   ├── tauri.conf.json     # App config (windows, bundle, plugins)
│   ├── capabilities/       # Security capabilities (JSON/TOML)
│   │   └── default.json
│   ├── icons/              # App icons (all sizes)
│   ├── binaries/           # External binaries (sidecars)
│   └── src/
│       ├── main.rs         # Entry point (calls lib::run)
│       └── lib.rs          # App setup, commands, plugins
├── package.json
└── vite.config.ts
```

---

## 3. Configuration

### `tauri.conf.json` — Key Sections

```jsonc
{
  "productName": "My App",
  "version": "0.1.0",
  "identifier": "com.company.myapp",   // Reverse-domain, required
  "build": {
    "devUrl": "http://localhost:1420",   // Vite dev server
    "frontendDist": "../dist"           // Built frontend assets
  },
  "app": {
    "windows": [
      {
        "title": "My App",
        "width": 1024,
        "height": 768,
        "decorations": true,
        "transparent": false
      }
    ],
    "security": {
      "csp": "default-src 'self'; script-src 'self'"
    }
  },
  "bundle": {
    "active": true,
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/icon.icns"],
    "targets": "all",                   // or ["dmg", "app", "msi", "deb"]
    "externalBin": ["binaries/my-sidecar"]  // Without target triple
  },
  "plugins": {}                         // Plugin-specific config
}
```

### Environment Variable Overrides

| Variable | Overrides |
|----------|-----------|
| `TAURI_SIGNING_PRIVATE_KEY` | Updater private key |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Key password |
| `APPLE_CERTIFICATE` | macOS signing cert (base64) |
| `APPLE_CERTIFICATE_PASSWORD` | Cert password |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Name (ID)` |
| `APPLE_ID` / `APPLE_PASSWORD` | Notarization credentials |
| `APPLE_TEAM_ID` | Apple Developer team ID |

---

## 4. IPC — Commands & Events

### Commands (Request-Response, Type-Safe)

**Rust side:**
```rust
#[tauri::command]
async fn greet(name: String) -> Result<String, String> {
    Ok(format!("Hello, {}!", name))
}

// Register in builder:
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![greet])
```

**Frontend side:**
```typescript
import { invoke } from '@tauri-apps/api/core';
const result = await invoke<string>('greet', { name: 'World' });
```

**Commands can access:**
```rust
#[tauri::command]
async fn do_work(
    window: tauri::WebviewWindow,   // Current window
    app: tauri::AppHandle,          // App handle
    state: tauri::State<'_, MyState>, // Managed state
) -> Result<String, String> { ... }
```

### Events (Fire-and-Forget, One-Way)

**Rust → Frontend:**
```rust
use tauri::Emitter;
app.emit("download-progress", 50)?;                    // All windows
app.emit_to("main", "download-complete", payload)?;     // Specific window
```

**Frontend listener:**
```typescript
import { listen } from '@tauri-apps/api/event';
const unlisten = await listen<number>('download-progress', (event) => {
    console.log(`Progress: ${event.payload}%`);
});
unlisten(); // Cleanup
```

### Channels (Streaming Data)

For high-throughput data (progress, file streaming):
```rust
#[tauri::command]
async fn stream_data(channel: tauri::ipc::Channel<Vec<u8>>) {
    for chunk in data.chunks(1024) {
        channel.send(chunk.to_vec()).unwrap();
    }
}
```

### When to Use Which

| Pattern | Use For |
|---------|---------|
| **Commands** | Request-response, type-safe, error handling |
| **Events** | Notifications, multiple consumers, no return value |
| **Channels** | Streaming, high throughput, progress updates |

---

## 5. State Management

```rust
use std::sync::Mutex;

struct AppState {
    counter: Mutex<i32>,
}

// In setup:
app.manage(AppState { counter: Mutex::new(0) });

// In command:
#[tauri::command]
fn increment(state: tauri::State<'_, AppState>) -> i32 {
    let mut counter = state.counter.lock().unwrap();
    *counter += 1;
    *counter
}
```

For async state, use `tokio::sync::Mutex` instead.

---

## 6. Sidecars (External Binaries)

### Configuration

In `tauri.conf.json`:
```json
{
  "bundle": {
    "externalBin": ["binaries/ivory-sidecar"]
  }
}
```

### Naming Convention

Binary files must include the **target triple**:
```
binaries/
└── ivory-sidecar-aarch64-apple-darwin    # macOS ARM
└── ivory-sidecar-x86_64-apple-darwin     # macOS Intel
└── ivory-sidecar-x86_64-pc-windows-msvc.exe  # Windows
```

Get the target triple: `rustc --print host-tuple`

The config references the base name only (`binaries/ivory-sidecar`). Tauri appends the target triple at build time.

### Spawning from Rust

```rust
use tauri_plugin_shell::ShellExt;

let sidecar_cmd = app.shell()
    .sidecar("ivory-sidecar")
    .expect("failed to create sidecar command")
    .args(["--port", &port.to_string()]);

let (mut rx, child) = sidecar_cmd.spawn()
    .expect("failed to spawn sidecar");

// Read stdout/stderr:
tauri::async_runtime::spawn(async move {
    while let Some(event) = rx.recv().await {
        match event {
            tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                println!("sidecar stdout: {}", String::from_utf8_lossy(&line));
            }
            tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                eprintln!("sidecar stderr: {}", String::from_utf8_lossy(&line));
            }
            _ => {}
        }
    }
});
```

### Required Plugin & Permissions

```toml
# Cargo.toml
tauri-plugin-shell = "2"
```

```json
// capabilities/default.json
{
  "permissions": [
    "shell:allow-spawn",
    "shell:allow-stdin-write"
  ]
}
```

### Sidecar Scope Configuration

```json
// capabilities/default.json
{
  "permissions": [
    {
      "identifier": "shell:allow-spawn",
      "allow": [
        { "name": "binaries/ivory-sidecar", "sidecar": true }
      ]
    }
  ]
}
```

---

## 7. Security & Capabilities

### Three-Layer Model

```
Capabilities → Permissions → Command Execution
```

1. **Capabilities** (in `src-tauri/capabilities/`): Define which permissions apply to which windows
2. **Permissions**: Control which commands can be called and with what scope
3. **Commands**: The actual Rust functions invoked via IPC

### Capability Definition

```json
// src-tauri/capabilities/default.json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-spawn",
    "fs:default"
  ]
}
```

### Per-Window Security

Different windows can have different permissions:
```json
// capabilities/admin.json
{
  "identifier": "admin-capability",
  "windows": ["admin"],
  "permissions": ["fs:all", "shell:execute"]
}

// capabilities/user.json
{
  "identifier": "user-capability",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

### Custom Permission Scopes

```toml
# src-tauri/permissions/file-ops.toml
[default]
description = "File system operations"
commands.allow = ["read_file", "write_file"]

[[scopes]]
allow = ["$HOME/Documents/*"]
deny = ["$HOME/Documents/.secret/*"]
```

### Plugin Permission Format

```
<plugin-name>:default         # Default set
<plugin-name>:<command-name>  # Specific command
<plugin-name>:allow-<cmd>     # Explicit allow
<plugin-name>:deny-<cmd>      # Explicit deny
```

### CSP Configuration

```json
{
  "app": {
    "security": {
      "csp": "default-src 'self'; script-src 'self'; connect-src 'self' http://localhost:*"
    }
  }
}
```

### Best Practices

- Validate all data crossing the IPC boundary in Rust
- Use least-privilege: only grant permissions each window needs
- Configure CSP to restrict script/resource loading
- All capabilities in `src-tauri/capabilities/` are auto-enabled

---

## 8. System Tray

### Enable Feature

```toml
# Cargo.toml
tauri = { version = "2", features = ["tray-icon"] }
```

### Create Tray with Menu (Rust)

```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton},
};

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    TrayIconBuilder::new()
        .menu(&menu)
        .menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    w.show().ok();
                    w.set_focus().ok();
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|_tray, event| match event {
            TrayIconEvent::Click { button: MouseButton::Left, .. } => {
                // Toggle window visibility
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
```

### Create Tray (JavaScript)

```typescript
import { TrayIcon } from '@tauri-apps/api/tray';
import { Menu } from '@tauri-apps/api/menu';

const menu = await Menu.new({
    items: [
        { id: 'show', text: 'Show Window' },
        { id: 'quit', text: 'Quit' },
    ],
});

const tray = await TrayIcon.new({
    menu,
    menuOnLeftClick: true,
    icon: 'icons/icon.png',
});
```

---

## 9. Window Customization

### Custom Titlebar (No Native Decorations)

```json
// tauri.conf.json
{ "app": { "windows": [{ "decorations": false }] } }
```

```html
<div data-tauri-drag-region class="titlebar">
  <button onclick="window.close()">✕</button>
</div>
```

Required permission: `core:window:allow-start-dragging`

Note: `data-tauri-drag-region` only works on the element it's directly applied to — not children.

### Transparent Titlebar (macOS)

Requires `cocoa` crate for NSWindow manipulation:
```rust
#[cfg(target_os = "macos")]
{
    use cocoa::appkit::NSWindow;
    // Configure transparent titlebar
}
```

---

## 10. Plugins

### Official Plugins (Common)

| Plugin | Package | Use |
|--------|---------|-----|
| Shell | `tauri-plugin-shell` | Spawn processes, sidecars, open URLs |
| FS | `tauri-plugin-fs` | File system access |
| Dialog | `tauri-plugin-dialog` | File picker, message dialogs |
| HTTP | `tauri-plugin-http` | HTTP client |
| Store | `tauri-plugin-store` | Persistent key-value store |
| Notification | `tauri-plugin-notification` | Desktop notifications |
| Clipboard | `tauri-plugin-clipboard-manager` | Read/write clipboard |
| Updater | `tauri-plugin-updater` | Auto-update support |
| Log | `tauri-plugin-log` | Structured logging |
| Process | `tauri-plugin-process` | App process info, restart, exit |

### Installing a Plugin

```bash
# Rust side
cargo add tauri-plugin-shell

# JavaScript side
pnpm add @tauri-apps/plugin-shell
```

```rust
// lib.rs
tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
```

```json
// capabilities/default.json — add permissions
{ "permissions": ["shell:default"] }
```

---

## 11. App Icons

### Required Sizes

Tauri needs icons in multiple sizes. Generate from a 1024×1024 source:

```bash
pnpm tauri icon path/to/icon.png
```

This generates all required sizes in `src-tauri/icons/`:
- `32x32.png`, `128x128.png`, `128x128@2x.png` (macOS/Linux)
- `icon.icns` (macOS)
- `icon.ico` (Windows)
- `Square*Logo.png` (Windows Store)

### Configuration

```json
{
  "bundle": {
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

---

## 12. Building & Bundle Formats

### Build Command

```bash
pnpm tauri build                    # All configured targets
pnpm tauri build --bundles app      # macOS .app only
pnpm tauri build --bundles dmg      # macOS .dmg only
pnpm tauri build --bundles msi      # Windows .msi
pnpm tauri build --bundles deb      # Linux .deb
pnpm tauri build --bundles appimage # Linux AppImage
```

### Output Locations

```
src-tauri/target/release/bundle/
├── macos/
│   ├── MyApp.app/
│   └── MyApp.dmg
├── msi/
│   └── MyApp_0.1.0_x64.msi
├── deb/
│   └── my-app_0.1.0_amd64.deb
└── appimage/
    └── my-app_0.1.0_amd64.AppImage
```

### Platform-Specific Bundles

| Platform | Formats | Notes |
|----------|---------|-------|
| **macOS** | `.app`, `.dmg` | DMG is drag-to-install. `.app` can be zipped |
| **Windows** | `.msi`, `.nsis` (`.exe`) | MSI for enterprise, NSIS for consumer |
| **Linux** | `.deb`, `.rpm`, `.AppImage` | AppImage is universal, no install needed |

### Bundle with Sidecar

The sidecar is automatically included when listed in `bundle.externalBin`. Tauri copies the correct platform-specific binary into the bundle.

### Build for Specific Target

```bash
# Cross-compile (requires proper toolchain)
pnpm tauri build --target x86_64-apple-darwin    # Intel Mac
pnpm tauri build --target aarch64-apple-darwin   # Apple Silicon
pnpm tauri build --target universal-apple-darwin # Universal (both)
```

---

## 13. Code Signing

### macOS

**Requirements:**
- Apple Developer account ($99/year)
- "Developer ID Application" certificate
- Notarization (required for Gatekeeper)

**Environment variables:**
```bash
export APPLE_CERTIFICATE="base64-encoded-p12"
export APPLE_CERTIFICATE_PASSWORD="password"
export APPLE_SIGNING_IDENTITY="Developer ID Application: Name (TEAMID)"
export APPLE_ID="dev@example.com"
export APPLE_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="TEAMID"
```

Then `pnpm tauri build` will automatically sign and notarize.

**Without signing (test distribution):**
Users must bypass Gatekeeper:
```bash
xattr -cr "/Applications/My App.app"
```

### Windows

Uses Authenticode signing. Can use OV or EV certificates.

```bash
export TAURI_SIGNING_PRIVATE_KEY="path/to/key.pfx"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="password"
```

### Linux

AppImages can be signed with GPG. `.deb` and `.rpm` use standard package signing.

---

## 14. Auto-Updater

### Setup

```bash
cargo add tauri-plugin-updater
pnpm add @tauri-apps/plugin-updater
```

```rust
tauri::Builder::default()
    .plugin(tauri_plugin_updater::Builder::new().build())
```

### Configuration

```json
// tauri.conf.json
{
  "plugins": {
    "updater": {
      "endpoints": ["https://releases.myapp.com/{{target}}/{{arch}}/{{current_version}}"],
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ..."
    }
  }
}
```

### Generate Signing Keys

```bash
pnpm tauri signer generate -w ~/.tauri/myapp.key
```

### Check for Updates (Frontend)

```typescript
import { check } from '@tauri-apps/plugin-updater';

const update = await check();
if (update) {
    await update.downloadAndInstall();
    // Restart app
}
```

### Update Server Response Format

The endpoint must return JSON:
```json
{
  "version": "0.2.0",
  "notes": "Bug fixes",
  "pub_date": "2026-03-05T00:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "signature": "...",
      "url": "https://releases.myapp.com/MyApp.app.tar.gz"
    }
  }
}
```

---

## 15. CI/CD (GitHub Actions)

### Build & Release Workflow

```yaml
name: Release
on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        include:
          - platform: macos-latest
            target: aarch64-apple-darwin
          - platform: macos-latest
            target: x86_64-apple-darwin
          - platform: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
          - platform: windows-latest
            target: x86_64-pc-windows-msvc

    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 20 }
      - uses: dtolnay/rust-toolchain@stable
      - run: pnpm install
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          # macOS signing
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
        with:
          tagName: v__VERSION__
          releaseName: 'v__VERSION__'
          releaseBody: 'See CHANGELOG.md'
```

### Pre-build Sidecar in CI

If your sidecar needs to be built before `tauri build`:
```yaml
- name: Build sidecar
  run: |
    cd src-tauri/sidecar
    pip install pyinstaller
    pyinstaller sidecar.spec --noconfirm
    TARGET=$(rustc --print host-tuple)
    cp dist/ivory-sidecar ../binaries/ivory-sidecar-$TARGET
```

---

## 16. App Size Optimization

### Rust Optimizations

```toml
# Cargo.toml
[profile.release]
strip = true          # Strip debug symbols
lto = true            # Link-time optimization
opt-level = "s"       # Optimize for size (or "z" for even smaller)
codegen-units = 1     # Better optimization, slower build
panic = "abort"       # Smaller binary, no unwinding
```

### Typical Sizes

| Component | Size |
|-----------|------|
| Tauri binary (Rust) | 5–15 MB |
| WebView | 0 MB (uses OS WebView) |
| Frontend assets | 1–5 MB |
| Sidecar (PyInstaller) | 30–80 MB |

### UPX Compression

For sidecars: `upx --best ivory-sidecar` can reduce binary by 30-50%.

---

## 17. Debugging

### Dev Mode

```bash
pnpm tauri dev      # Hot-reload + DevTools
```

- **Frontend**: Open WebView DevTools with right-click → Inspect (or `Cmd+Option+I` on macOS)
- **Rust**: Use `println!` or `dbg!` — output appears in terminal
- **Environment**: Set `RUST_LOG=debug` for verbose Tauri logs

### Debug Build

```bash
pnpm tauri build --debug
```

Produces a debug build with DevTools enabled. Useful for testing the release bundle with debugging capabilities.

### Console Logging from Rust

```rust
use tauri_plugin_log::{Builder as LogBuilder, Target, TargetKind};

tauri::Builder::default()
    .plugin(
        LogBuilder::default()
            .targets([
                Target::new(TargetKind::Stdout),
                Target::new(TargetKind::LogDir { file_name: None }),
            ])
            .build(),
    )
```

---

## 18. Common Pitfalls

| Pitfall | Solution |
|---------|----------|
| Sidecar not found at runtime | Name must include target triple: `binary-aarch64-apple-darwin` |
| "Not allowed" IPC error | Add permission to `capabilities/default.json` |
| macOS Gatekeeper blocks app | Sign + notarize, or tell users `xattr -cr` |
| `invoke` returns undefined | Command must return `Result<T, String>` (not just `T`) |
| State not accessible in command | Must call `app.manage(state)` before commands use it |
| Sidecar stdin not working | Need `shell:allow-stdin-write` permission |
| Frontend can't connect to localhost | Add `connect-src http://localhost:*` to CSP |
| DMG build fails on macOS beta | Use `--bundles app` and zip manually |
| PyInstaller sidecar crashes | Check for `inspect.getsource()` calls — exclude problematic packages |
| Universal binary build fails | Build each arch separately, then use `lipo` to combine |

---

## Ivory-Specific Patterns

These patterns are specific to how Ivory uses Tauri:

### Sidecar Lifecycle
1. Tauri Core picks a random port
2. Spawns `ivory-sidecar --port <N> --data-dir <app_data_dir>`
3. Health-checks `GET /health/ready` until responsive
4. Frontend proxies all API calls through Tauri commands to `http://localhost:<N>`

### Build Sequence
```bash
# 1. Build Python sidecar
cd src-tauri/sidecar && uv run pyinstaller sidecar.spec --noconfirm

# 2. Copy with target triple name
cp dist/ivory-sidecar ../binaries/ivory-sidecar-$(rustc --print host-tuple)

# 3. Build Tauri app
cd ../.. && pnpm tauri build --bundles app

# 4. Package for distribution
cd src-tauri/target/release/bundle/macos
zip -r "Elefant-Ivory-0.1.0-aarch64.zip" "Elefant - Ivory.app"
```

### Backend Mode Routing
- **Ollama** (default): All LLM calls go through sidecar → Ollama on localhost
- **BYOK**: Sidecar uses user's API key to call cloud providers
- **Premium**: Calls routed to hosted API (future)
