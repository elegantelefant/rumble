# How This App Works

## 1. Current CleanSweep Tauri Architecture

The existing CleanSweep app demonstrates a minimalist Tauri build that keeps the user-facing experience focused and approachable.

### 1.1 Desktop Shell & Command Bridge
- **Runtime**: The app boots as a Tauri desktop process. `src-tauri/src/lib.rs` registers two commands, `scan_folder` and `move_to_trash`, with `tauri::Builder::default().invoke_handler(...)`, exposing them to the frontend through the secure IPC bridge provided by Tauri commands.[^tauri-command]
- **Invocation flow**: Vue components call `invoke("scan_folder", { path })`, which marshals a message over the IPC boundary, runs the Rust handler, and resolves back into JavaScript via promises. This pattern keeps Rust logic encapsulated while giving the UI a single asynchronous touchpoint.

### 1.2 Rust Backend Responsibilities
- **Filesystem scanning**: `scan_folder` walks the requested directory with `walkdir`, collects metadata, and returns an array of `FileInfo` structs serialised with Serde.
- **Trash operations**: `move_to_trash` converts the supplied path to a `PathBuf` and calls the `trash` crate, aligning with the platform recycle bin.
- **Sidecar expectations**: CleanSweep does not yet spawn a sidecar, but the architecture leaves room to wire a Python process later using Tauri’s sidecar APIs without altering the frontend contract.[^tauri-sidecar]

### 1.3 Vue Frontend Structure
- **Single-file component**: `App.vue` owns the entire UI. It stores `path`, `files`, and `loading` as `ref`s and wires them into three child components.
- **Child components**:
  - `PathSelector.vue` manages the path input and triggers scans.
  - `DiskUsageSummary.vue` renders aggregate metrics using computed helpers.
  - `CleanupSuggestions.vue` lists the top files and emits `trash` events.
- **Design**: Tailwind utility classes keep styling declarative. The Tailwind config restricts content paths and uses the same typography/spacing as the React tutorial, preserving the calm visual tone.

### 1.4 Shared Contracts & Utilities
- **Type safety**: `src/types.ts` exports the `FileInfo` TypeScript definition, mirroring the Rust struct.
- **Utility helpers**: `src/utils.ts` provides `formatSize`, `oldestFileDate`, and `largestFile`, ensuring both the Vue UI and any future modules display data consistently.
- **Simplicity**: All state lives in the root component; communication is via props/emits only. No global store or complex routing is introduced, keeping the mental model linear.

### 1.5 Data Flow Overview
```
User action → Vue component emits @scan/@trash → invoke("scan_folder" | "move_to_trash")
           → Rust handler performs filesystem work → returns FileInfo[] or ()
           → Vue updates refs → components re-render
```

## 2. Elefant Toolbox Frontend Plan (Vue + Tailwind, Tauri-friendly)

The Elefant Toolbox evolves CleanSweep into a multi-tool legal assistant while keeping the same emphasis on clarity, minimalism, and desktop-native feel.

### 2.1 Guiding Principles
- **Preserve simplicity**: Maintain the flat state patterns, explicit data contracts, and Tailwind-first styling ethos from CleanSweep.
- **Desktop-first**: Design for a 1024px+ viewport with a persistent sidebar; progressively enhance for smaller widths using Tailwind responsive utilities.
- **Command-first architecture**: Every AI-facing interaction still funnels through `invoke`. The frontend remains agnostic about whether Rust handles a filesystem scan or proxies to a Python FastAPI sidecar.

### 2.2 Application Skeleton
- **Entry point**: Continue mounting from `src/main.ts`, now importing a router.
- **Layout components**:
  - `AppShell.vue`: Hosts the sidebar, top bar, and content outlet. Accepts props for sidebar position, user info, and app version.
  - `SidebarNav.vue`: Renders the Elefant/Ivory tool list, reflecting active/disabled/premium states via props. Uses simple `@click` events to push router navigation.
  - `TopBar.vue`: Houses breadcrumbs, contextual actions, and global status (sync indicator, notifications). It now renders the `BrandLogo` placeholder (Elefant primary wordmark with Ivory tag) so a production asset can drop in later.
- **Routing**: Use Vue Router with routes `/login`, `/review`, `/draft`, `/research`, `/translation`, `/evals`, `/briefcases`, `/settings`. Evidence Review and Plugins routes render placeholder “coming soon” cards matching the spec.

### 2.3 Page Implementations
- **Login (`/login`)**: A centered card component `LoginView.vue` with the Ivory brand mark, passphrase input, remember checkbox, and CTA button. Form submission emits `loginAttempt(passphrase)` and awaits an `invoke("auth_login", {...})` call. Errors and loading states mirror CleanSweep alerts but with inline feedback.
- **Document Review (`/review`)**: Split view with an upload drop zone (`FileDropZone.vue`) and chat area (`ChatPanel.vue`).
  - **Data flow**: Upload emits `invoke("documents_add", files)`. Questions call `invoke("documents_query", { conversationId, question })`. Responses render `ChatMessage` components with citation badges.
- **Document Draft (`/draft`)**: Two-column layout; left column lists templates (`TemplateList.vue`), right column hosts `DraftForm.vue`. Generating a draft calls `invoke("draft_generate", { templateId, params })` and streams updates to show progress.
- **Research (`/research`)**: Similar chat layout but emphasises saved threads using a `ThreadsSidebar.vue`. Queries invoke `research_query`. Results highlight citations.
- **Translation (`/translation`)**: Dual-pane component `TranslationWorkbench.vue` with language selectors, text areas, and disclaimers. Submit invokes `translation_run`.
- **Evals (`/evals`)**: Dashboard view with checklist controls (`ModelSelector.vue`, `BenchmarkCard.vue`) and a `ResultsChart.vue` for summary. Buttons fire `invoke("evals_run", payload)` and display progress.
- **Briefcases (`/briefcases`)**: Presents subscription upsell and, when unlocked, lists synced cases using `BriefcaseList.vue`.
- **Settings (`/settings`)**: Tabbed form capturing API keys, folder paths, appearance, and sync options. Save emits `invoke("settings_update", formState)`.

### 2.4 Shared UI Library
- **Primitives**: Extract reusable components (`Button.vue`, `InputField.vue`, `Card.vue`, `Badge.vue`, `SkeletonLoader.vue`) mirroring the design system tokens. Tailwind classes use CSS variables defined on `:root` to keep theming unified.
- **Icons**: Wrap Lucide icons in `Icon.vue` that accepts a name prop, keeping usage consistent.
- **Feedback**: Provide `ToastService` and `ModalProvider` via provide/inject to avoid heavy dependencies.

### 2.5 State Management Strategy
- **Global store**: Introduce a lightweight Pinia store (or Vue `reactive` singleton) for session/user metadata, current tool state, and cached documents.
- **Per-tool modules**: Each route owns its module with fetch/command helpers, keeping logic close to the UI and preserving CleanSweep’s small surface area per feature.
- **Invoke helpers**: Centralise IPC calls in `src/ipc/client.ts`, exposing typed functions like `scanFolder(path: string): Promise<FileInfo[]>`. This mirrors CleanSweep’s clarity while making the growing command surface easy to audit.

### 2.6 Interaction With Rust & Python
- **Command contract**: For every AI interaction the Vue UI calls `invoke("tool_action", payload)`. Rust handlers validate, orchestrate filesystem/database access, and forward LLM work to the FastAPI sidecar over HTTP. Responses travel back through the same promise interface.
- **Streaming**: Where partial updates are needed (draft generation, long research responses), adopt Tauri’s `Event` API to emit progress from Rust to Vue while keeping the request minimal.[^tauri-events]
- **Error handling**: Standardise on a `ResultEnvelope` type so the Vue layer can show concise alerts without leaking backend implementation details.

### 2.7 Keeping the Experience Elegant
- **Progressive disclosure**: Default screens show essential controls first; advanced toggles live behind accordions or secondary buttons.
- **Consistent spacing**: Apply the provided spacing scale via Tailwind (`gap-4`, `px-6`, etc.) to ensure modules feel cohesive.
- **Keyboard shortcuts**: Use the Vue lifecycle to register shortcuts (`Cmd+K` for command palette, `Cmd+/` for help) that call `invoke("ui_shortcut", { action })`, letting Rust log or act accordingly.

### 2.8 Deliverables Checklist
- Component skeletons for all routes with placeholder content and Ivory rebranding. ✅
- IPC client module with stubbed command wrappers. ☐ (next)
- Tailwind theme file defining the Ivory/ELEFANT tokens. ✅
- Documentation comments in components describing the expected command interaction. ☐ (add inline as wiring matures)

## 3. BetterAuth Login Implementation Plan

The current `LoginView.vue` supplies the Elefant-first surface; the next iteration wires it into a BetterAuth-powered flow that still respects Tauri’s command boundary. The diagram below summarises the data path:

```
LoginView → useAuth composable → invoke("auth_login")
                 ↓
            Rust (BetterAuth client)
                 ↓
         SQLCipher session store
                 ↓
     (optional) FastAPI sidecar
```

1. **Frontend composable (`src/modules/auth/useAuth.ts`)**
   - Expose `signIn(passphrase, remember)` that calls `invoke("auth_login", { passphrase, remember })` and stores the returned BetterAuth session (token + expiry) in a reactive store (`useAuthStore`).
   - Provide `signOut()` and `getSession()` helpers that hit `invoke("auth_logout")` / `invoke("auth_session")`, enabling auto-refresh on app launch.
   - Surface `authState.status` (`"idle" | "authenticating" | "authenticated" | "error"`) so the login button and future global banners can reflect state without bespoke logic scattered around components.

2. **Route guards**
   - In `router.ts`, add a navigation guard that redirects unauthenticated users to `/login`, leveraging `await invoke("auth_session")` to validate the BetterAuth session whenever Tauri rehydrates.
   - Keep `/login` accessible without auth; after a successful sign-in push to `/review`. Optionally store the intended route and resume once auth completes.

3. **Rust command layer**
   - Implement `auth_login`, `auth_logout`, and `auth_session` commands in `src-tauri/src/lib.rs`. Each command calls into a Rust BetterAuth client (configured with Elefant policies) which stores encrypted session data in SQLCipher.
   - Use Tauri’s `State` to cache session context; emit events (`auth://session-changed`) for the Vue app to update instantly. Tauri docs on commands and managed state remain applicable.[^tauri-command]

4. **Python sidecar coordination (future)**
   - When delegating to the FastAPI sidecar, forward the BetterAuth session token as an HTTP header so the sidecar can enforce the same auth rules.
   - Sidecar responses should surface `401`/`403` so Rust can translate them into `Err` results, prompting the Vue layer to redirect to `/login`.

5. **UX polish**
   - Replace the current timeout stub in `LoginView.vue` with the composable’s `signIn` call; show BetterAuth error codes inline (e.g., lockouts, expired passphrase).
   - Swap the placeholder “LOGO” badge with an SVG asset when delivered; because `BrandLogo` already accepts a compact variant, the top bar and login card stay in sync.

This plan keeps the login experience minimal while ensuring BetterAuth operates through Tauri’s secure IPC, matching Ivory’s local-first philosophy.

---

[^tauri-command]: Tauri Commands documentation — https://tauri.app/v1/guides/features/command/
[^tauri-sidecar]: Tauri Sidecar Processes — https://tauri.app/v1/guides/features/processes/
[^tauri-events]: Tauri Events & Emitters — https://tauri.app/v1/guides/features/events/

