# Rumble — Selenium Test Implementation Plan

How to implement each test from `spec.md`. Each task is a checkbox. Check it off when implemented AND passing.

---

## Infrastructure

- [x] I.1 Shared helpers in `selenium-tests/test/helpers.js`: `buildApp()`, `startDriver()`, `stopDriver()`, `getDriver()`, `navigateTo(path)`, `waitFor(css)`, `waitForText(text)`
- [x] I.2 Root hooks in `selenium-tests/test/root-hooks.js`: build once, start/stop driver once for entire suite
- [x] I.3 `.mocharc.yml` discovers all test files, excludes helpers, sets 120s timeout
- [x] I.4 All test files use ESM (`import`), match `test/*.js` glob

---

## Test File: `test/smoke.js` (Spec §1)

- [x] T1.1 `it("window renders content")` — `findElement(By.css("h1")).getText()`, assert non-empty
- [x] T1.2 `it("page title is set")` — `driver.getTitle()`, assert includes "Elefant"

---

## Test File: `test/app-shell.js` (Spec §2–4)

### Sidebar (Spec §2)

- [x] T2.1 `it("sidebar visible with branding")` — find `<aside>`, getText, assert "Elefant", "Rumble", "Workspace"
- [x] T2.2 `it("shows all navigation items")` — assert aside text includes all 5 tool labels + Settings
- [x] T2.3 `it("evidence review marked coming soon")` — assert aside text includes "Coming Soon"
- [x] T2.4 `it("shows user info card")` — assert "CoastalTower238", "SilverEcho951", "Rumble v0.1.0a"
- [x] T2.5 `it("shows confidentiality tagline")` — assert aside text includes "confidential ai tools" (case-insensitive, CSS uppercases)
- [x] T2.6 `it("navigates to each active route")` — loop through 5 routes, click sidebar button via XPath `//aside//button[.//div[text()='${label}']]`, wait for h1 to match expected heading
- [x] T2.7 `it("evidence review click does not navigate")` — navigate to /review, click evidence, assert h1 still "Document Review"
- [x] T2.8 `it("sidebar reorder")` — click move-down arrow on first item, verify order changed. Check first item Up arrow is `disabled`. Check last item Down arrow is `disabled`.

### TopBar (Spec §3)

- [x] T3.1 `it("shows confidentiality label")` — find `<header>`, assert text includes "local & confidential" (case-insensitive)
- [x] T3.2 `it("shows shortcuts button")` — find `header button[aria-label="Open shortcuts"]`

### Command Palette (Spec §4)

- [x] T4.1 `it("opens via Cmd+K")` — send Meta+K via `driver.actions()`, wait for `[role="dialog"][aria-label="Command palette"]`
- [x] T4.2 `it("lists commands in listbox")` — find `[role="listbox"]`, count `[role="option"]` > 0
- [x] T4.3 `it("search filters commands")` — type "doc" in `#palette-search`, assert all remaining options contain "doc"
- [x] T4.4 `it("closes on Escape")` — send Escape to search input, wait for transition, assert dialog gone from DOM
- [x] T4.5 `it("no matches message")` — reopen, type "zzz", assert "No matches" text visible
- [x] T4.6 `it("arrow keys move highlight")` — open, send ArrowDown, verify `aria-selected="true"` moves
- [x] T4.7 `it("enter navigates and closes")` — highlight an item, press Enter, assert palette closes and page navigated
- [x] T4.8 `it("click option navigates")` — open, click an option, assert palette closes

---

## Test File: `test/document-review.js` (Spec §5)

- [x] T5.1 `before()` — `navigateTo("/review")`
- [x] T5.2 `it("page heading")` — h1 text equals "Document Review"
- [x] T5.3 `it("trust badge")` — main text matches /confidential|local|device/i
- [x] T5.4 `it("file upload drop zone")` — main text matches /drag|drop|browse|upload/i
- [x] T5.5 `it("pre-seeded session")` — main text includes "Contract_2024.pdf"
- [x] T5.6 `it("custom prompt textarea")` — find textarea, count > 0
- [x] T5.7 `it("click session shows chat")` — click "Contract_2024.pdf" element, wait 600ms for mock, assert main text grew
- [x] T5.8 `it("chat input exists")` — find text input or textarea
- [x] T5.9 `it("API surface toggle")` — main text matches /api|surface/i

---

## Test File: `test/document-draft.js` (Spec §6)

- [x] T6.1 `before()` — `navigateTo("/draft")`
- [x] T6.2 `it("page heading")` — "Document Draft"
- [x] T6.3 `it("three templates")` — assert "Employment", "Non-Disclosure", "Service" in main text
- [x] T6.4 `it("template selectable")` — click Employment, assert form field text matches /employee|name|salary/i
- [x] T6.5 `it("form fields have labels")` — count `<label>` elements > 0
- [x] T6.6 `it("generate button")` — assert /generate|draft/i in main text
- [x] T6.7 `it("export buttons")` — assert /word|pdf|export/i in main text
- [x] T6.8 `it("privacy notice")` — assert /privacy|local|confidential|device/i

---

## Test File: `test/research.js` (Spec §7)

- [x] T7.1 `before()` — `navigateTo("/research")`
- [x] T7.2 `it("page heading")` — "Research Assistant"
- [x] T7.3 `it("pre-populated threads")` — main text length > 50
- [x] T7.4 `it("model selector")` — assert /ollama|model/i in main text
- [x] T7.5 `it("new thread button")` — assert /new thread/i in main text
- [x] T7.6 `it("prompt textarea")` — count textareas > 0
- [x] T7.7 `it("API key requirement")` — assert /api key/i in main text
- [x] T7.8 `it("click thread activates")` — click first thread button, assert content updates

---

## Test File: `test/translation.js` (Spec §8)

- [x] T8.1 `before()` — `navigateTo("/translation")`
- [x] T8.2 `it("page heading")` — "Translation"
- [x] T8.3 `it("pre-populated job")` — assert /french|english|fr|en/i in main text
- [x] T8.4 `it("language dropdowns")` — count `<select>` >= 2
- [x] T8.5 `it("source textarea")` — count textareas > 0
- [x] T8.6 `it("translate button")` — assert /translate/i in main text
- [x] T8.7 `it("draft quality warning")` — assert /draft|quality|disclaimer/i
- [x] T8.8 `it("jurisdiction disclaimer")` — assert /jurisdiction|legal|accuracy/i

---

## Test File: `test/settings.js` (Spec §9)

- [x] T9.1 `before()` — `navigateTo("/settings")`
- [x] T9.2 `it("page heading")` — "Settings"
- [x] T9.3 `it("four tabs visible")` — assert "Providers", "Templates" (or "Storage"), "Appearance", "Sync" in main text
- [x] T9.4 `it("tab switching")` — click Sync tab, assert /sync|server|connection/i; click Appearance tab, assert /theme|sidebar|position/i
- [x] T9.5 `it("providers — Ollama entry")` — click Providers tab, assert /ollama/i
- [x] T9.6 `it("providers — OpenAI/Anthropic")` — assert /openai/i and /anthropic/i
- [x] T9.7 `it("providers — local config")` — assert /host|port|model/i
- [x] T9.8 `it("sync — enable checkbox")` — count `input[type="checkbox"]` > 0
- [x] T9.9 `it("sync — team code")` — check input value contains "SilverEcho951"
- [x] T9.10 `it("sync — test connection button")` — assert /test connection/i
- [x] T9.11 `it("save button")` — assert /save/i

---

## Test Files: Future (Spec §10–11)

- [ ] T10.1 Toast verification — trigger a settings save, wait for green toast element, assert auto-dismiss after 3s
- [ ] T10.2 Multiple toasts — trigger two actions in quick succession, assert 2 toast elements
- [ ] T11.1 KeepAlive: Document Review — type in chat input, navigate to settings, navigate back, assert text preserved
- [ ] T11.2 KeepAlive: Research — type in prompt textarea, navigate away and back, assert preserved
- [ ] T11.3 KeepAlive: Translation ��� type in source area, navigate away and back, assert preserved

---

## Bug Fixes

> When a bug is found during testing (logged in `spec.md` Bugs Encountered), add the fix task here.

| # | Bug (from spec) | Fix Description | File(s) | Status |
|---|-----------------|-----------------|---------|--------|
| | | | | |
