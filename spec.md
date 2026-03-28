# Rumble — Selenium E2E Test Specification

What to test. Each checkbox is a test case. Check it off when the selenium test passes.

**Legend:**
- [ ] Not yet tested
- [x] Selenium test passes (with screenshots in `e2e-screenshots/`)
- [!] Selenium test fails — add bug to Bugs Encountered below

**Every test must screenshot before/after every interaction and assertion.** Screenshots are saved to `e2e-screenshots/` with auto-incrementing names. A test without screenshots is incomplete.

---

## 1. Smoke — App Launches

- [x] 1.1 App window renders, at least one `<h1>` is present with non-empty text
- [x] 1.2 Page title contains "Elefant"

---

## 2. AppShell — Sidebar

- [x] 2.1 Sidebar (`<aside>`) is visible and contains "Elefant" and "Rumble"
- [x] 2.2 Sidebar contains "Workspace" label
- [x] 2.3 All 5 tool nav items present: Document Review, Research, Document Draft, Evidence Review, Translation
- [x] 2.4 Settings nav item present
- [x] 2.5 Evidence Review shows "Coming Soon" sublabel
- [x] 2.6 User info card shows "CoastalTower238"
- [x] 2.7 User info card shows team "SilverEcho951"
- [x] 2.8 Version label shows "Rumble v0.1.0a"
- [x] 2.9 Confidentiality tagline present: "Confidential AI Tools"
- [x] 2.10 Clicking each active nav item navigates to correct page (Document Review, Research, Document Draft, Translation, Settings)
- [x] 2.11 Clicking Evidence Review does NOT navigate away from current page
- [x] 2.12 Sidebar reorder: move first tool down, order changes
- [x] 2.13 Sidebar reorder: first item Up arrow is disabled
- [x] 2.14 Sidebar reorder: last item Down arrow is disabled

---

## 3. AppShell — TopBar

- [x] 3.1 TopBar shows "Local & Confidential" label
- [x] 3.2 Shortcuts button (`aria-label="Open shortcuts"`) is present
- [x] 3.3 User avatar chip shows initials

---

## 4. AppShell — Command Palette

- [x] 4.1 Cmd+K opens palette dialog (`role="dialog"`, `aria-label="Command palette"`)
- [x] 4.2 Palette contains listbox with at least 1 option
- [x] 4.3 Typing in search input filters options (e.g. "doc" filters to doc-related commands)
- [x] 4.4 Filtered results all contain the search term
- [x] 4.5 Escape closes the palette
- [x] 4.6 Palette is gone from DOM after close
- [x] 4.7 Arrow Down/Up moves highlight (`aria-selected`)
- [x] 4.8 Enter on highlighted item navigates + closes palette
- [x] 4.9 Clicking an option navigates + closes palette
- [x] 4.10 Typing "zzz" shows "No matches" message

---

## 5. Document Review (`/review`)

- [x] 5.1 Page heading is "Document Review"
- [x] 5.2 Trust/confidentiality badge visible (text matches "confidential", "local", or "device")
- [x] 5.3 File upload drop zone visible (text matches "drag", "drop", "browse", or "upload")
- [x] 5.4 Pre-seeded session "Contract_2024.pdf" is visible
- [x] 5.5 Custom prompt textarea exists
- [x] 5.6 Clicking pre-seeded session shows chat workspace
- [x] 5.7 Chat input field exists when session is active
- [x] 5.8 API surface toggle button is present
- [x] 5.9 Clicking toggle expands/collapses API surface panel
- [ ] 5.10 Sending empty question does nothing (no new message appears)
- [ ] 5.11 Session card shows filename, size, and status badge

---

## 6. Document Draft (`/draft`)

- [x] 6.1 Page heading is "Document Draft"
- [x] 6.2 Three templates listed: Employment, Non-Disclosure, Service
- [x] 6.3 Clicking Employment template shows employment-related form fields
- [x] 6.4 Form fields have labels
- [x] 6.5 "Generate Draft" button exists
- [x] 6.6 Export buttons present (Word/PDF)
- [x] 6.7 Data privacy notice visible
- [ ] 6.8 Clicking NDA template switches context (heading changes)
- [ ] 6.9 Clicking "Generate Draft" with empty fields shows validation errors

---

## 7. Research (`/research`)

- [x] 7.1 Page heading is "Research Assistant"
- [x] 7.2 Pre-populated research threads visible
- [x] 7.3 Model selector present, Ollama shown as default
- [x] 7.4 "New Thread" button exists
- [x] 7.5 Prompt textarea exists
- [x] 7.6 Hosted models show "API key" requirement text
- [x] 7.7 Clicking a thread activates it (content area updates)
- [ ] 7.8 Thread status badges are visible (colors for in-progress, complete)
- [ ] 7.9 Submitting empty prompt does nothing

---

## 8. Translation (`/translation`)

- [x] 8.1 Page heading is "Translation"
- [x] 8.2 Pre-populated translation job visible (French/English reference)
- [x] 8.3 Source language dropdown exists with multiple options
- [x] 8.4 Target language dropdown exists with multiple options
- [x] 8.5 Source text area exists
- [x] 8.6 Translate button exists
- [x] 8.7 Draft quality warning visible
- [x] 8.8 Jurisdiction disclaimer visible
- [ ] 8.9 Model selector present

---

## 9. Settings (`/settings`)

- [x] 9.1 Page heading is "Settings"
- [x] 9.2 Four tabs visible: Providers, Templates/Storage, Appearance, Sync
- [x] 9.3 Clicking each tab switches displayed content
- [x] 9.4 Providers tab: pre-populated Ollama entry visible
- [x] 9.5 Providers tab: OpenAI and Anthropic options listed
- [x] 9.6 Providers tab: local config fields (host/port/model) shown for Ollama
- [x] 9.7 Sync tab: enable checkbox present
- [x] 9.8 Sync tab: team code "SilverEcho951" visible
- [x] 9.9 Sync tab: "Test Connection" button present
- [ ] 9.10 Appearance tab: theme/sidebar position options exist
- [x] 9.11 Save button exists

---

## 10. Cross-Cutting — Toast System

- [x] 10.1 Triggering an action that produces a success toast shows green notification
- [x] 10.2 Toast auto-dismisses after ~3 seconds
- [x] 10.3 Multiple simultaneous toasts stack independently

---

## 11. Navigation State Preservation (KeepAlive)

- [x] 11.1 Enter text in Document Review chat, navigate to Settings, navigate back — text is preserved
- [x] 11.2 Enter text in Research prompt, navigate away and back — text is preserved
- [x] 11.3 Enter text in Translation source area, navigate away and back — text is preserved

---

## Bugs Encountered

> When a test fails or unexpected behavior is found, log it here. Add a corresponding fix task to `imp_plan.md`.

| # | Spec Item | Severity | Description | Status |
|---|-----------|----------|-------------|--------|
| | | | | |
