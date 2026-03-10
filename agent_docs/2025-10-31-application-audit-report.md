ABOUTME: rumble principal engineer audit log.
ABOUTME: appendix capturing page interactions, issues, and remediation plan.

# Application Audit Report

## MASTER PAGE CHECKLIST
- [x] Document Review (`src/views/DocumentReviewView.vue`)
- [x] Document Draft (`src/views/DocumentDraftView.vue`)
- [x] Research Assistant (`src/views/ResearchView.vue`)
- [x] Translation (`src/views/TranslationView.vue`)
- [x] Model Evals (`src/views/EvalsView.vue`)
- [x] Briefcases (`src/views/BriefcasesView.vue`)
- [x] Evidence Review (`src/views/EvidenceReviewView.vue`)
- [x] Plugins Marketplace (`src/views/PluginsView.vue`)
- [x] Settings (`src/views/SettingsView.vue`)
- [x] Login (`src/views/LoginView.vue`)
- [x] App Shell & Navigation (`src/layouts/AppShell.vue`, `src/modules/navigation`)

## DETAILED INTERACTION AUDIT

### PAGE: Document Review (`src/views/DocumentReviewView.vue`)

#### Interactions Checklist:
- [x] Drag/drop or browse uploads
  - User action → drop zone `@drop` / browse button `@click` (`src/views/DocumentReviewView.vue:247`, `:265`) → `handleFiles` (`127`) → `registerFile` (`105`) → `queueInitialReview` (`172`) → mock backend `mockRegisterReview`/`mockInitialReview`
  - Status: ✓ WORKS (stubbed backend responds with summary and initial assistant message)
  - Notes: Documents filtered to PDF/DOCX/TXT; prompt persisted per session.
- [x] Custom prompt entry
  - Textarea input (`src/views/DocumentReviewView.vue:284`) → stored in `customPrompt` and passed during `registerFile`
  - Status: ✓ WORKS
- [x] Active session selection
  - Click item (`src/views/DocumentReviewView.vue:332`) → `openSession` (`165`) → `beginInitialReview` (`144`)
  - Status: ✓ WORKS
- [x] Ask follow-up question
  - Form submit (`src/views/DocumentReviewView.vue:439`) → `askQuestion` (`196`) → pushes user message and awaits `mockDocumentChat`
  - Status: ✓ WORKS (mock answer without citations)
- [x] API surface toggle
  - Button (`src/views/DocumentReviewView.vue:236`) → `showApiDocs` flag
  - Status: ✓ WORKS; shows expectations registry.
- [ ] Export session
  - Button rendered (`src/views/DocumentReviewView.vue:417`) but no handler attached.
  - Status: ⚠️ ISSUE — missing export logic. Severity: 🟡 MEDIUM. Expectation: call backend to package chat + summary.

### PAGE: Document Draft (`src/views/DocumentDraftView.vue`)

#### Interactions Checklist:
- [x] Template selection
  - Buttons (`src/views/DocumentDraftView.vue:42`) toggle `selectedTemplate`
  - Status: ✓ WORKS
- [ ] Manage templates CTA
  - Header button (`src/views/DocumentDraftView.vue:31`) lacks click handler.
  - Status: ⚠️ ISSUE — no navigation or modal. Severity: 🟢 LOW.
- [ ] Browse local templates
  - Button (`src/views/DocumentDraftView.vue:58`) has no event binding.
  - Status: ⚠️ ISSUE — should open file picker. Severity: 🟢 LOW.
- [x] Form validation on blur
  - Inputs call `validateField` (`src/views/DocumentDraftView.vue:18`)
  - Status: ✓ WORKS
- [x] Generate draft
  - Button (`src/views/DocumentDraftView.vue:104`) → `generateDraft` (`34`) → mock toast after validation
  - Status: ✓ WORKS (stubbed)
- [ ] Export actions
  - Buttons (`src/views/DocumentDraftView.vue:108-109`) → `exportDraft` (`48`) -> toast only
  - Status: ⚠️ ISSUE — missing file generation. Severity: 🟡 MEDIUM.

### PAGE: Research Assistant (`src/views/ResearchView.vue`)

#### Interactions Checklist:
- [x] API surface toggle
  - Button (`src/views/ResearchView.vue:199`) toggles doc list from `backendRegistry`
  - Status: ✓ WORKS
- [x] New thread
  - Button (`src/views/ResearchView.vue:228`) → `startNewThread` (`92`)
  - Status: ✓ WORKS
- [x] Thread selection
  - Buttons (`src/views/ResearchView.vue:233`) → `activateThread` (`169`)
  - Status: ✓ WORKS
- [x] Submit research prompt
  - Button (`src/views/ResearchView.vue:210`) → `submitPrompt` (`131`) → `mockResearchRun`
  - Status: ✓ WORKS (mock memo appended with citations)
- [ ] Model availability messaging
  - Only local model selectable; hosted options disabled (`src/views/ResearchView.vue:120`).
  - Status: ⚠️ ISSUE — UI should flag missing API keys inline (currently only disabled). Severity: 🟢 LOW.

### PAGE: Translation (`src/views/TranslationView.vue`)

#### Interactions Checklist:
- [x] API surface toggle (`src/views/TranslationView.vue:133`)
- [x] History selection (`src/views/TranslationView.vue:153`) → `activateJob` (`67`)
- [x] Model selector (`src/views/TranslationView.vue:177`) enforces availability
- [x] Translate button
  - Button (`src/views/TranslationView.vue:208`) → `runTranslation` (`78`)
  - Status: ✓ WORKS (mock response, toast, history)
- [ ] Output download/export
  - Not implemented; users cannot save translated text aside from copy.
  - Status: ⚠️ ISSUE — add download or clipboard support. Severity: 🟢 LOW.

### PAGE: Model Evals (`src/views/EvalsView.vue`)

#### Interactions Checklist:
- [x] API surface toggle (`src/views/EvalsView.vue:114`)
- [x] Model toggles (`src/views/EvalsView.vue:143`) → `toggleModel` (`94`)
- [x] Run quick benchmarks (`src/views/EvalsView.vue:185`) → `runBenchmark` (`99`)
  - Status: ✓ WORKS (mock completion, history append)
- [x] Custom benchmark queue (`src/views/EvalsView.vue:228`) → `scheduleCustomBenchmark` (`148`)
  - Status: ✓ WORKS (adds placeholder history)
- [ ] Export CSV / Detailed report buttons (`src/views/EvalsView.vue:208`)
  - Status: ⚠️ ISSUE — no handlers. Severity: 🟡 MEDIUM.
- [ ] Attach to briefcase CTA (`src/views/EvalsView.vue:257`)
  - Status: ⚠️ ISSUE — no click logic. Severity: 🟢 LOW.

### PAGE: Briefcases (`src/views/BriefcasesView.vue`)

#### Interactions Checklist:
- [ ] Join waitlist button (`src/views/BriefcasesView.vue:14`)
  - Status: ⚠️ ISSUE — lacks handler. Severity: 🟢 LOW.
- Rest of view informational only.

### PAGE: Evidence Review (`src/views/EvidenceReviewView.vue`)
- Informational placeholder, no interactions — mark as not applicable.

### PAGE: Plugins Marketplace (`src/views/PluginsView.vue`)
- Informational placeholder, no interactions.

### PAGE: Settings (`src/views/SettingsView.vue`)

#### Interactions Checklist:
- [x] Tab navigation (`src/views/SettingsView.vue:161`) toggles `activeTab`
- [x] Add provider secret (`src/views/SettingsView.vue:180`) → `addSecret` (`107`)
- [x] Save settings (`src/views/SettingsView.vue:246`) → `saveSettings` (`204` → `mockSaveSettings`)
- [x] Toggle custom sync + test connection (`src/views/SettingsView.vue:230`, `252`) → `testSync` (`212`) → `mockTestSync`
- [ ] Browse buttons (`src/views/SettingsView.vue:189`) and briefcase/actions (`202`, `217`)
  - Status: ⚠️ ISSUE — UI hints but lacks handlers. Severity: 🟢 LOW.

### PAGE: Login (`src/views/LoginView.vue`)

#### Interactions Checklist:
- [x] Passphrase form submission → `handleSubmit` (`src/views/LoginView.vue:11`)
  - Status: ✓ WORKS via mock delay (navigates to `/review`)
- [ ] “Generate phrase” link (`src/views/LoginView.vue:61`)
  - Status: ⚠️ ISSUE — no handler. Severity: 🟢 LOW.

### Layout & Navigation (`src/layouts/AppShell.vue`, `src/modules/navigation`)

#### Interactions Checklist:
- [x] Sidebar toggle (button + ⌘ + `) (`src/layouts/AppShell.vue:48`)
- [x] Shortcuts palette (button + ⌘K) (`src/layouts/AppShell.vue:61`, `src/components/CommandPalette.vue`)
- [x] Sidebar reordering controls (`src/modules/navigation/SidebarNav.vue:120`)
  - Status: ✓ WORKS (state local only; persists until reload)
- [x] API confidentiality badge updating (static placeholder, but logic ready for hybrid states).

## CODE QUALITY FINDINGS

### Critical Issues
- None observed. Build passes and primary flows function with stubs.

### Medium Issues
- Document Review export button lacks implementation (`src/views/DocumentReviewView.vue:417`).
- Document Draft export buttons emit toast only (`src/views/DocumentDraftView.vue:48`, `108`).
- Model Evals export controls missing behavior (`src/views/EvalsView.vue:208`).

### Low Issues
- Numerous CTA placeholders without handlers (Manage Templates, Browse Templates, Attach to Briefcase, etc.).
- Research model selector simply disables unavailable entries; should surface guidance inline.
- Translation view lacks explicit export/save affordance.
- Sidebar reordering resets on reload (state not persisted).

### Complexity Analysis
- `src/views/DocumentReviewView.vue` — Stateful but cohesive; consider extracting session store if backend streaming introduced.
- `src/views/SettingsView.vue` — Large component managing multiple concerns; future refactor into sub-components per tab recommended.
- `src/modules/backend/backendClient.ts` — Centralized mock/stub definitions keep expectations discoverable; acceptable while backend absent.

## IMPLEMENTATION PLAN

### Phase 1: Critical Fixes (0 min)
- No blocking issues identified.

### Phase 2: Quick Wins (≈ 3.5 hours total)

#### Fix #1: Implement Document Export Actions (90 min)
- File: `src/views/DocumentReviewView.vue:417`
- Problem: Export button inert; users cannot archive sessions.
- Solution: Wire to new `mockExportSession` (or real backend invoke) returning packaged JSON/ZIP; trigger download.
- Steps:
  1. Add client helper in `backendClient.ts` (`mockExportSession(sessionId)` returning blob URL placeholder).
  2. Attach click handler to export button invoking helper and using `URL.createObjectURL`.
  3. Add toast/error handling.
- Testing: Upload demo file, click export, verify download triggered.
- Risk: 🟡 Low — ensure URL revoked to avoid leaks.

#### Fix #2: Enable Draft Exports (75 min)
- File: `src/views/DocumentDraftView.vue:48`
- Problem: `exportDraft` only emits toast.
- Solution: Integrate with backend or generate client-side DOCX/PDF stub; until backend ready, create downloadable text file summarizing form input.
- Steps: build payload, call helper, download.
- Testing: Complete form, export to both formats, confirm file contents.
- Risk: 🟢 None.

#### Fix #3: Hook Template/Briefcase CTAs (45 min)
- Files: `src/views/DocumentDraftView.vue:31`, `src/views/SettingsView.vue:189`, `202`, `217`, `src/views/BriefcasesView.vue:14`
- Problem: Buttons lack handlers leading to dead UI ends.
- Solution: Add stub modals or navigation to instruct users until feature ready; track telemetry.
- Steps: attach `@click` with placeholder dialogues referencing upcoming functionality.
- Testing: Click each CTA, ensure feedback.
- Risk: 🟢 None.

#### Fix #4: Surface Model Availability Guidance (30 min)
- File: `src/views/ResearchView.vue:120`
- Problem: Disabled options offer no inline reason.
- Solution: Add tooltip or inline helper text referencing Settings ➝ Providers.
- Steps: conditionally render caption; update tests.
- Risk: 🟢 None.

#### Fix #5: Provide CSV/Report Exports in Evals (70 min)
- File: `src/views/EvalsView.vue:208`
- Problem: Export buttons inert.
- Solution: Serialize `benchmarkHistory` to CSV / Markdown report; use download link.
- Steps: create helpers, bind buttons, reuse mock data until backend responses real.
- Testing: Run quick benchmark, export, verify file contents.
- Risk: 🟡 Low — ensure large datasets handled.

### Phase 3: Medium Effort (≈ 6 hours total)

#### Fix #6: Persist Sidebar Ordering & Settings
- Files: `src/modules/navigation/SidebarNav.vue:120`, `src/layouts/AppShell.vue`
- Problem: Reordering lost on reload.
- Solution: Store ordering in localStorage (or future backend preference endpoint) and hydrate on mount.
- Steps: add persistence layer, handle validation, include migration for new tools.
- Testing: Reorder, reload, confirm order.
- Risk: 🟡 Low — guard against corrupted storage.

#### Fix #7: Integrate Backend Service Layer (Document, Research, Translation, Evals)
- Files: `src/modules/backend/backendClient.ts`, associated views.
- Problem: Current mocks block product parity with spec.
- Solution: Replace `mock*` calls with real `invoke` wrappers; maintain documentation within client file.
- Steps: define typed wrappers, update views, add error states.
- Testing: Unit test wrappers, manual flow verification with backend.
- Risk: 🔴 High — depends on backend readiness and streaming semantics.

#### Fix #8: Modularize Settings Tabs
- File: `src/views/SettingsView.vue`
- Problem: Single component handles multiple domains.
- Solution: Extract providers/storage/appearance/sync into scoped child components with typed props/emits.
- Steps: create `SettingsProviders.vue`, etc., update layout.
- Testing: Regression test each tab, ensure `saveSettings` aggregates child state.
- Risk: 🟡 Medium — coordinate with future backend persistence work.

### Total Estimated Effort
- Phase 2: ~3.5 hours
- Phase 3: ~6 hours
- Combined (excluding backend integration risks): ~9.5 hours

## Testing Checklist (Post-fixes)
- Upload + export document session
- Generate draft + export Word/PDF
- Run research query with unavailable model guidance
- Translate text and download output
- Run benchmark and export CSV/report
- Verify sidebar ordering persists after reload
- Smoke test Settings flows (add secret, save, sync test)

## Backend Expectations Summary
- Documented in `src/modules/backend/backendClient.ts` and surfaced via API toggle UI for each feature.
- Real implementation should replace mocks with IPC commands matching documented payloads.
