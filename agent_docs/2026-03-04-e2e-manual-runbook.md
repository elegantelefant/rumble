# Rumble — Manual E2E Testing Runbook

> **Tester:** _______________
> **Date:** _______________
> **Build/Commit:** _______________
> **OS:** _______________
> **Ollama running:** [ ] Yes / [ ] No

**Legend:** Mark each item as you go:
- [x] Pass
- [!] Fail — add notes inline
- [-] Skipped — add reason
- [ ] Not yet tested
- [?] Predicted issue from code review — verify manually

---

## Phase 0 — Prerequisites

| # | Step | Status | Notes |
|---|------|--------|-------|
| 0.1 | Rust toolchain installed (`rustc --version`) | [ ] | |
| 0.2 | Node + pnpm installed (`pnpm --version`) | [ ] | |
| 0.3 | Python + uv installed (`uv --version`) | [ ] | |
| 0.4 | Ollama installed and running (`ollama list`) | [ ] | |
| 0.5 | At least one Ollama model pulled (e.g. `llama3.2`) | [ ] | Model: __________ |

---

## Phase 1 — Build & Launch

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 1.1 | `pnpm install` in project root | Dependencies install without errors | LIKELY OK | [ ] | |
| 1.2 | `cd src-tauri/sidecar && uv sync` | Python venv created, deps installed | LIKELY OK | [ ] | |
| 1.3 | `pnpm tauri dev` | App window opens (800x600), title "Elefant - Rumble" | LIKELY OK — Vite + Tauri config look correct | [ ] | |
| 1.4 | System tray icon appears | "rumble" icon visible in menu bar | LIKELY OK — icon embedded at compile time, but no click handler (tray is inert) | [ ] | |
| 1.5 | No white screen / crash on launch | Vue app renders inside the window | LIKELY OK | [ ] | |
| 1.6 | Check terminal: sidecar starts | `PORT:<n>` printed, health check passes | **WILL FAIL** — sidecar binary is an 11-byte stub (`#!/bin/sh`). Must build real binary via `uv run pyinstaller sidecar.spec`, or run sidecar manually with `pnpm dev:sidecar` | [ ] | |
| 1.7 | App lands on `/review` | Document Review page is visible | LIKELY OK — auth guard hardcoded to `true`, root redirects to `/review` | [ ] | |

### Phase 1 — Known Issues from Code Review

| ID | Severity | Issue |
|----|----------|-------|
| B1.1 | **BLOCKER** | `src-tauri/binaries/rumble-sidecar-aarch64-apple-darwin` is an 11-byte stub. No build script to produce the real PyInstaller binary. Sidecar will not start. Workaround: run `pnpm dev:sidecar` in a separate terminal. |
| B1.2 | Medium | `sidecar.spec` is missing `routes.jobs` and `services.jobs` in `hiddenimports` — when a real binary IS built, the `/draft`, `/review`, `/research` endpoints will fail with ImportError. |
| B1.3 | Medium | Sidecar writes `rumble.db` to CWD. When launched from Tauri, CWD may not be writable. Rust does not pass `--data-dir`. |
| B1.4 | Low | `PORT:` is printed before uvicorn binds the socket — health polls may fail on slow cold starts (10 retries × 500ms = 5s budget). |
| B1.5 | Low | Default `BackendMode` is `Premium` — all API calls route to `api.elefant.com`, not the sidecar. Must call `set_backend_mode("ollama")` to use local mode. |

---

## Phase 2 — Login Flow

> **Note:** Auth is currently stubbed (`isAuthenticated = true`). Test what's wired up.

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 2.1 | Navigate to `/login` manually | Redirects back to `/review` | LIKELY OK — guard always returns true | [ ] | |
| 2.2 | Set `isAuthenticated = false` in router, reload | Login page renders | LIKELY OK | [ ] | |
| 2.3 | Submit with empty passphrase | "Passphrase required" error appears | LIKELY OK — but error renders BELOW the "Generate phrase" button, odd placement | [ ] | |
| 2.4 | "Remember for 30 days" checkbox | Default checked; toggles on click | LIKELY OK | [ ] | |
| 2.5 | Submit with any passphrase | Button shows "Authenticating...", navigates to `/review` | **CHECK** — spinner uses CSS class `loading` but rest of app uses `spinner`. May not animate. | [ ] | |
| 2.6 | "Generate phrase" button | Button renders | **NO HANDLER** — button has no `@click`, completely inert, no user feedback on click | [ ] | |
| 2.7 | Restore `isAuthenticated = true` | Back to normal | OK | [ ] | |

---

## Phase 3 — AppShell Layout

### 3a. Sidebar

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 3.1 | Sidebar visible on desktop | Logo, heading, tool list, user card | LIKELY OK | [ ] | |
| 3.2 | Nav items present | 5 tools listed | LIKELY OK | [ ] | |
| 3.3 | Click "Document Review" | Navigates to `/review`, item highlighted | LIKELY OK | [ ] | |
| 3.4 | Click "Research" | Navigates to `/research`, highlighted | LIKELY OK | [ ] | |
| 3.5 | Click "Document Draft" | Navigates to `/draft`, highlighted | LIKELY OK | [ ] | |
| 3.6 | Click "Translation" | Navigates to `/translation`, highlighted | LIKELY OK | [ ] | |
| 3.7 | Click "Evidence Review" | Nothing happens | LIKELY OK — `aria-disabled` + guard in handler | [ ] | |
| 3.8 | Click "Settings" | Navigates to `/settings` | LIKELY OK | [ ] | |
| 3.9 | Reorder: move first tool down | Order changes | LIKELY OK — splice + reassign triggers reactivity | [ ] | |
| 3.10 | Reorder: move last tool up | Order changes | LIKELY OK | [ ] | |
| 3.11 | First item Up arrow disabled | Cannot move up | LIKELY OK | [ ] | |
| 3.12 | Last item Down arrow disabled | Cannot move down | LIKELY OK | [ ] | |
| 3.13 | User card at bottom | "CoastalTower238", "SilverEcho951", "Rumble v0.1.0a" | LIKELY OK — hardcoded values | [ ] | |

### 3b. TopBar

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 3.14 | Confidentiality label | "Local & Confidential" visible | LIKELY OK — hardcoded, no toggle exists | [ ] | |
| 3.15 | "⌘ K Shortcuts" button | Present on desktop | LIKELY OK | [ ] | |
| 3.16 | User avatar chip | "CT" initials | LIKELY OK — hardcoded, same values as sidebar (duplicated) | [ ] | |

### 3c. Command Palette

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 3.17 | Press Cmd+K | Palette opens, input focused | LIKELY OK — `v-if` triggers autofocus on mount | [ ] | |
| 3.18 | All commands listed | 3 commands visible | LIKELY OK | [ ] | |
| 3.19 | Type "doc" | Filters to matching items | LIKELY OK | [ ] | |
| 3.20 | Type "zzz" | "No matches" message | LIKELY OK | [ ] | |
| 3.21 | Arrow Down/Up | Highlight moves | LIKELY OK | [ ] | |
| 3.22 | Enter on highlighted item | Navigates + closes | **CHECK** — if user types to filter (narrowing list), `highlightedIndex` is NOT clamped. Pressing Enter on a stale index silently does nothing. Must arrow-key first to reset. | [ ] | |
| 3.23 | Escape | Closes, no navigation | LIKELY OK | [ ] | |
| 3.24 | Click an item | Navigates + closes | LIKELY OK | [ ] | |
| 3.25 | Click TopBar "⌘ K" button | Palette opens | LIKELY OK — but note: clicking this button only opens, never closes. Only Cmd+K toggles. | [ ] | |
| 3.26 | Cmd+K again while open | Closes | LIKELY OK — AppShell handler toggles | [ ] | |

### 3d. Mobile / Responsive (resize to < 768px)

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 3.27 | Sidebar hidden by default | Only topbar + content | LIKELY OK — `md:translate-x-0` / `-translate-x-full` | [ ] | |
| 3.28 | Hamburger visible | ☰ icon | LIKELY OK — `md:hidden` class | [ ] | |
| 3.29 | Click hamburger | Sidebar slides in | LIKELY OK | [ ] | |
| 3.30 | Click backdrop | Sidebar closes | LIKELY OK — `@click="sidebarOpen = false"` on overlay | [ ] | |
| 3.31 | "⌘ K" button hidden on mobile | Not visible | LIKELY OK — `hidden md:flex` | [ ] | |

---

## Phase 4 — Document Review (`/review`)

### 4a. File Upload

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 4.1 | Drop zone visible | Drag text + Browse button | LIKELY OK | [ ] | |
| 4.2 | Drag PDF over drop zone | Visual hover state | LIKELY OK | [ ] | |
| 4.3 | Drop a PDF file | Session created, review starts | LIKELY OK — uses mock backend | [ ] | |
| 4.4 | Click "Browse Files" | Native file picker | LIKELY OK | [ ] | |
| 4.5 | Multiple files via picker | Multiple sessions | LIKELY OK | [ ] | |
| 4.6 | Custom prompt textarea | Can type before upload | LIKELY OK | [ ] | |
| 4.7 | Custom prompt clears after upload | Textarea empties | LIKELY OK | [ ] | |

### 4b. Session List

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 4.8 | Pre-loaded session | "Contract_2024.pdf" visible | LIKELY OK — hardcoded initial data | [ ] | |
| 4.9 | Session card details | Filename, size, badge, timestamp | LIKELY OK | [ ] | |
| 4.10 | Click session | Activates, shows chat | LIKELY OK | [ ] | |
| 4.11 | New upload status flow | "Reviewing" → "Summary ready" | LIKELY OK — but if `mockInitialReview` throws, session permanently stuck at "running" with no error feedback (no try/catch) | [ ] | |

### 4c. Chat Workspace

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 4.12 | No session selected | Placeholder text | LIKELY OK | [ ] | |
| 4.13 | Session header | File details + badge | LIKELY OK | [ ] | |
| 4.14 | Review summary | Summary text after ~550ms | LIKELY OK with mock | [ ] | |
| 4.15 | Message thread | Messages + timestamps | LIKELY OK | [ ] | |
| 4.16 | Follow-up question + Send | User msg → assistant reply | **CHECK** — no try/catch on `mockDocumentChat`. If it throws, user message stays but no reply appears, input not cleared, no error shown. Also: no disabled state on Send button during flight — user can spam. | [ ] | |
| 4.17 | Send empty question | Nothing happens | LIKELY OK — guard checks `question.value.trim()` | [ ] | |
| 4.18 | "Export Session" button | Present | **NO HANDLER** — button has no `@click`, completely inert | [ ] | |

### 4d. API Surface Toggle

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 4.19 | Toggle button visible | Present | LIKELY OK | [ ] | |
| 4.20 | Click toggle | Panel expands/collapses | LIKELY OK | [ ] | |

### Phase 4 — Known Issues from Code Review

| ID | Severity | Issue |
|----|----------|-------|
| B4.1 | High | `askQuestion` has no error handling — if backend call fails, conversation is left in broken state (user message without reply, input not cleared) |
| B4.2 | High | `queueInitialReview` has no error handling — session permanently stuck at "running" on failure |
| B4.3 | Medium | Send button has no disabled/loading state — user can spam Send while request is in flight, causing duplicate messages |
| B4.4 | Medium | `isSyncingBackend` is shared across all sessions — concurrent uploads show misleading sync indicator |

---

## Phase 5 — Document Draft (`/draft`)

### 5a. Template Library

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 5.1 | Three templates listed | Employment, NDA, Service Contract | LIKELY OK | [ ] | |
| 5.2 | Click a template | Highlighted border | LIKELY OK | [ ] | |
| 5.3 | Click different template | Highlight moves | **BUG** — heading shows "Selected" instead of actual template name for NDA and Service Contract | [ ] | |
| 5.4 | "Browse Local Templates..." button | Present | **NO HANDLER** — completely inert | [ ] | |

### 5b. Form Validation

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 5.5 | Tab through empty required fields | Inline errors | LIKELY OK | [ ] | |
| 5.6 | Fill field, tab away | Error clears | LIKELY OK | [ ] | |
| 5.7 | Generate Draft with empty fields | All errors shown | LIKELY OK | [ ] | |
| 5.8 | All fields filled + Generate | Spinner → success toast | LIKELY OK with mock | [ ] | |
| 5.9 | Button disabled while generating | Cannot re-click | LIKELY OK | [ ] | |

### 5c. Export

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 5.10 | Export to Word | Info toast | LIKELY OK | [ ] | |
| 5.11 | Export to PDF | Info toast | LIKELY OK | [ ] | |

### Phase 5 — Known Issues from Code Review

| ID | Severity | Issue |
|----|----------|-------|
| B5.1 | Medium | Template heading shows "Selected" instead of template name for NDA/Service Contract (ternary only handles `"employment"`) |
| B5.2 | Medium | Form fields are always employment-specific (Employee Name, Salary, etc.) regardless of which template is selected. NDA/Service Contract show irrelevant fields. |
| B5.3 | Medium | "Manage Templates" button (header) has no handler — inert |
| B5.4 | Low | `isGenerating` not reset on error path — when real backend is wired, button could get permanently disabled |

---

## Phase 6 — Research Assistant (`/research`)

### 6a. Thread Management

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 6.1 | Pre-populated threads | Two threads visible | LIKELY OK | [ ] | |
| 6.2 | Click a thread | Activates, shows messages | LIKELY OK | [ ] | |
| 6.3 | Status badges | Correct colors | LIKELY OK | [ ] | |
| 6.4 | "New Thread" | Untitled thread at top | LIKELY OK | [ ] | |

### 6b. Research Execution

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 6.5 | Model selector | 3 models, only Ollama selectable | LIKELY OK | [ ] | |
| 6.6 | Hosted models disabled | "(add API key)" message | LIKELY OK | [ ] | |
| 6.7 | Empty prompt | Nothing happens | LIKELY OK | [ ] | |
| 6.8 | Submit with Ollama | Message → toast → reply with citations | LIKELY OK with mock | [ ] | |
| 6.9 | Thread status transitions | In Progress → Complete | LIKELY OK | [ ] | |
| 6.10 | Citation chips | Visible | LIKELY OK | [ ] | |
| 6.11 | No model selected | Error toast | **UNREACHABLE** — `selectedModel` is always initialized to the first available model (`elefant-local`). This guard can never fire. | [ ] | |

### Phase 6 — Known Issues from Code Review

| ID | Severity | Issue |
|----|----------|-------|
| B6.1 | **High** | "Start Research" button has no disabled/loading state. User can click multiple times during a request, spawning concurrent `mockResearchRun` calls that corrupt the thread with interleaved messages. |
| B6.2 | Low | `ensureModelAvailability` on `@change` is dead code — model is never empty after a user selection |

---

## Phase 7 — Translation (`/translation`)

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 7.1 | Pre-populated job | FR → EN visible | LIKELY OK | [ ] | |
| 7.2 | Click history item | Text populates | **UX** — silently discards any unsaved text in the editor with no confirmation | [ ] | |
| 7.3 | Source language dropdown | 6 options | LIKELY OK | [ ] | |
| 7.4 | Target language dropdown | 5 options | LIKELY OK | [ ] | |
| 7.5 | Model dropdown | Ollama available | LIKELY OK | [ ] | |
| 7.6 | Translate | Spinner → result → toast → history entry | LIKELY OK — `isTranslating` correctly disables button | [ ] | |
| 7.7 | Empty source | Nothing happens | LIKELY OK | [ ] | |
| 7.8 | Warning badge | "Draft Quality" visible | LIKELY OK | [ ] | |
| 7.9 | Info banner | Translation notes | LIKELY OK | [ ] | |

### Phase 7 — Known Issues from Code Review

| ID | Severity | Issue |
|----|----------|-------|
| B7.1 | Low | No validation that source and target languages differ — can translate English → English |
| B7.2 | Low | Clicking a history job silently discards unsaved editor input — no confirmation dialog |

---

## Phase 8 — Settings (`/settings`)

### 8a. Tab Navigation

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 8.1 | Four tab buttons | Providers, Templates, Appearance, Sync | LIKELY OK | [ ] | |
| 8.2 | Click each tab | Content switches | LIKELY OK | [ ] | |

### 8b. Providers Tab

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 8.3 | Pre-populated secret | Ollama entry visible | LIKELY OK | [ ] | |
| 8.4 | Select Ollama provider | Host/port/model fields | LIKELY OK | [ ] | |
| 8.5 | Select OpenAI provider | API key field | LIKELY OK | [ ] | |
| 8.6 | Save with OpenAI + no key | Error toast | LIKELY OK | [ ] | |
| 8.7 | Save with valid data | Entry added, form clears, toast | LIKELY OK | [ ] | |
| 8.8 | Remove secret | Entry disappears | LIKELY OK — no confirmation dialog | [ ] | |

### 8c. Templates Tab

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 8.9 | Templates folder path | Shows path | LIKELY OK | [ ] | |
| 8.10 | Workspace path | Shows path | LIKELY OK | [ ] | |
| 8.11 | Briefcases list | Empty on first run; saved names after Save + relaunch (#18) | LIKELY OK | [ ] | |

### 8d. Appearance Tab

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 8.12 | Sidebar position dropdowns | Left/Right options | LIKELY OK — persisted to settings.json on Save (#18), not yet applied to the layout | [ ] | |
| 8.13 | Theme dropdown | 3 options | LIKELY OK — persisted on Save (#18), not yet applied | [ ] | |
| 8.14 | "Show chat history" checkbox | Toggleable | LIKELY OK | [ ] | |

### 8e. Sync Tab

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 8.15 | "Enable secure sync" checkbox | Default checked | LIKELY OK | [ ] | |
| 8.16 | Team code input | "SilverEcho951" | LIKELY OK | [ ] | |
| 8.17 | Default server input | Disabled | LIKELY OK | [ ] | |
| 8.18 | "Use custom sync server" | Unchecked | LIKELY OK | [ ] | |
| 8.19 | Check custom server | URL input enabled | LIKELY OK | [ ] | |
| 8.20 | Test connection | Info toast | LIKELY OK — but ignores `mockTestSync`, uses hardcoded toast | [ ] | |

### 8f. Save

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 8.21 | Save settings | Spinner → toast | LIKELY OK — but uses raw `setTimeout`, not `mockSaveSettings` | [ ] | |

### Phase 8 — Known Issues from Code Review

| ID | Severity | Issue |
|----|----------|-------|
| B8.1 | Low | `saveSettings` uses raw `setTimeout` instead of `mockSaveSettings` — inconsistent with other views |
| B8.2 | Low | `testSync` ignores `mockTestSync` entirely — hardcoded toast |
| B8.3 | Low | ~~Settings values (appearance, sync config) are not persisted — lost on page refresh~~ Fixed by #18: saved to `settings.json` in the app local data dir, loaded when Settings opens |
| B8.4 | Low | Form is not disabled during save — user can change values while "saving" |

---

## Phase 9 — Toast System (cross-cutting)

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 9.1 | Success toast color | Green | LIKELY OK | [ ] | |
| 9.2 | Error toast color | Red | LIKELY OK | [ ] | |
| 9.3 | Info toast color | Blue/accent | LIKELY OK | [ ] | |
| 9.4 | Auto-dismiss | Disappears after ~3s | LIKELY OK — timer fires correctly, no early dismiss option | [ ] | |
| 9.5 | Multiple toasts stack | Independent dismiss | LIKELY OK | [ ] | |

### Phase 9 — Known Issues from Code Review

| ID | Severity | Issue |
|----|----------|-------|
| B9.1 | Low | No manual dismiss — `pointer-events-auto` suggests it was intended, but no `@click` handler exists |
| B9.2 | Low | Timer not cleaned up on unmount — stale timers fire into dead closures (no crash, but wasteful) |

---

## Phase 10 — Sidecar Health (via terminal)

> Requires sidecar running (see B1.1 — run `pnpm dev:sidecar` manually).

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 10.1 | `curl /health` | `{"status":"ok","mode":"ollama"}` | LIKELY OK | [ ] | |
| 10.2 | `curl /ready` | Ready status | **CHECK** — if Ollama is slow (>5s), `httpx.TimeoutException` is not caught → 500 instead of `not_ready` | [ ] | |
| 10.3 | `curl /models` | Model list | Same timeout issue as 10.2 | [ ] | |
| 10.4 | Create chat | `{"id":"..."}` | LIKELY OK | [ ] | |
| 10.5 | Send message | Assistant reply | **BUG** — multi-turn context is dropped. Only last message is sent to LLM; conversation history is fetched from DB but discarded. Every reply is context-free. | [ ] | |

### Phase 10 — Known Issues from Code Review

| ID | Severity | Issue |
|----|----------|-------|
| **B10.1** | **HIGH** | Multi-turn chat context is lost. `llm.py` sends only `messages[-1]["content"]` to the model — full conversation history is ignored. Chat has no memory. |
| B10.2 | Medium | `_parse_llm_json` fails when LLM returns markdown fence with trailing newline (`\`\`\`\n`). The `.endswith("\`\`\`")` check misses trailing whitespace → 502 on all AI endpoints. |
| B10.3 | Medium | `extra='forbid'` on response models — if LLM returns extra JSON keys (common), Pydantic raises ValidationError → 500 on `/clarify`, `/translate`, `/summarise/*`. |
| B10.4 | Medium | `httpx.TimeoutException` not caught in `/ready` and `/models` — slow Ollama causes 500 instead of graceful `not_ready`. |
| B10.5 | Medium | `ResearchResultResponse.sources` expects `SearchResult` objects with `id` and `title`, but LLM is unlikely to produce that shape → 500 on research completion. |

---

## Phase 11 — Shutdown & Cleanup

| # | Step | Expected | Prediction | Status | Notes |
|---|------|----------|------------|--------|-------|
| 11.1 | Close app window | App closes | LIKELY OK | [ ] | |
| 11.2 | No orphan sidecar | `ps aux` shows no process | **CHECK** — `kill_sidecar` only fires on `RunEvent::Exit`. If tray keeps app alive after window close, sidecar may linger. | [ ] | |
| 11.3 | Tray icon removed | Gone from menu bar | LIKELY OK | [ ] | |

---

## Summary

| Phase | Items | Predicted OK | Predicted Issues | Status |
|-------|-------|-------------|-----------------|--------|
| 0. Prerequisites | 5 | 5 | 0 | |
| 1. Build & Launch | 7 | 5 | 2 | |
| 2. Login | 7 | 5 | 2 | |
| 3. AppShell | 31 | 30 | 1 | |
| 4. Document Review | 20 | 16 | 4 | |
| 5. Document Draft | 11 | 7 | 4 | |
| 6. Research | 11 | 9 | 2 | |
| 7. Translation | 9 | 7 | 2 | |
| 8. Settings | 21 | 17 | 4 | |
| 9. Toasts | 5 | 5 | 0 | |
| 10. Sidecar | 5 | 2 | 3 | |
| 11. Shutdown | 3 | 2 | 1 | |
| **TOTAL** | **135** | **110** | **25** | |

---

## All Issues from Code Review (sorted by severity)

### BLOCKERS (must fix before testing)

| ID | Component | Issue |
|----|-----------|-------|
| B1.1 | Sidecar binary | Stub binary — sidecar will not start. Build with PyInstaller or run `pnpm dev:sidecar` manually. |

### HIGH (will cause visible broken behavior)

| ID | Component | Issue |
|----|-----------|-------|
| B10.1 | `llm.py` | Multi-turn chat context dropped — every message answered without memory |
| B10.2 | `ai.py` | `_parse_llm_json` fails on trailing newline after markdown fence → 502 |
| B10.3 | `ai.py` | `extra='forbid'` on response models → 500 when LLM returns extra keys |
| B4.1 | DocumentReview | `askQuestion` has no error handling — broken state on failure |
| B4.2 | DocumentReview | `queueInitialReview` has no error handling — session stuck forever |
| B6.1 | Research | "Start Research" has no loading guard — concurrent clicks corrupt thread |

### MEDIUM (UX degradation or edge-case failures)

| ID | Component | Issue |
|----|-----------|-------|
| B1.2 | `sidecar.spec` | Missing `hiddenimports` for `routes.jobs` / `services.jobs` |
| B1.3 | Sidecar | SQLite DB written to CWD — may be unwritable in production |
| B1.5 | Rust | Default mode is `Premium` — sidecar never receives requests until mode switched |
| B4.3 | DocumentReview | Send button not disabled during flight — spam creates duplicates |
| B5.1 | DocumentDraft | Template heading shows "Selected" for NDA/Service Contract |
| B5.2 | DocumentDraft | Form fields are always employment-specific regardless of template |
| B10.4 | `health.py` | `TimeoutException` not caught → 500 on slow Ollama |
| B10.5 | `jobs.py` | `ResearchResultResponse.sources` type mismatch → 500 |

### LOW (cosmetic, minor inconsistency, or future risk)

| ID | Component | Issue |
|----|-----------|-------|
| B1.4 | Sidecar | PORT printed before socket bound — timing assumption |
| B5.3 | DocumentDraft | "Manage Templates" / "Browse" buttons inert |
| B5.4 | DocumentDraft | `isGenerating` not reset on error |
| B6.2 | Research | `ensureModelAvailability` dead code |
| B7.1 | Translation | Can translate same language to same language |
| B7.2 | Translation | History click discards unsaved input silently |
| B8.1 | Settings | `saveSettings` uses raw setTimeout, not mock |
| B8.2 | Settings | `testSync` ignores mock function |
| B8.3 | Settings | ~~Settings not persisted across refresh~~ fixed (#18) |
| B8.4 | Settings | Form not disabled during save |
| B9.1 | Toasts | No manual dismiss despite pointer-events-auto |
| B9.2 | Toasts | Timer leak on unmount |

---

## Your Notes & Observations

> _Use this space as you test. Add rows, cross-reference issue IDs above._

### Confirmed Bugs

| ID | Confirmed? | Actual Behavior |
|----|-----------|-----------------|
| | | |

### New Issues Found (not in code review)

1.

### UX Friction

1.

### General Notes

1.
