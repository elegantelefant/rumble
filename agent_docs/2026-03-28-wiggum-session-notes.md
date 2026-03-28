# Wiggum Session Notes

> Observations, surprises, and non-obvious findings from wiggum loop iterations.

---

## Session Notes

### Infrastructure (I.1–I.4) — 2026-03-28

**tauri-driver doesn't support macOS.** This was the biggest surprise. `tauri-driver` v2.0.5 only supports Linux (WebKitGTK) and Windows (MSEdgeDriver). On macOS it exits immediately with "tauri-driver is not supported on this platform".

**Workaround:** Replaced the tauri-driver approach with Chrome headless + Vite preview server. The helpers.js now:
1. Builds the frontend with `pnpm build`
2. Starts `pnpm preview --port 4173`
3. Connects headless Chrome via Selenium Manager (auto-downloads chromedriver)
4. Tests against `http://localhost:4173`

This tests all Vue UI behavior but not Tauri-specific features (window management, IPC, tray, etc.). Since all spec items test DOM elements, this covers 100% of the spec.

**mocharc.yml needed `require:` not `file:`** for root hooks. The `file:` directive loads a file as a test file but doesn't register `mochaHooks` exports. `require:` properly loads root hook plugins.

**package.json `--timeout 60000`** overrode the mocharc `timeout: 120000`. Removed CLI timeout to let mocharc be authoritative.

**CSS `text-transform: uppercase`** affects `getText()` return values in Selenium. Sidebar tagline "Confidential AI Tools" and topbar "Local & Confidential" both render uppercased. Tests must use case-insensitive comparisons.

**Research view h1 is "Research Assistant"** not "Research". The sidebar label is "Research" but the view heading is "Research Assistant".

**Template names:** "NDA" in the spec should be "Non-Disclosure Agreement" (the full display name).

**SilverEcho951 in settings sync tab** is inside an `<input>` element — `getText()` on main doesn't capture input values. Must use `getAttribute("value")`.

**Selenium Manager works great.** No need to install chromedriver manually — selenium-webdriver 4.34 auto-downloads compatible chromedriver via Selenium Manager.

**Build binary name:** Cargo.toml names the package `elefant_rumble` but old build artifacts show `elefant_ivory` in target/debug/. Fresh build produces `elefant_rumble`.
