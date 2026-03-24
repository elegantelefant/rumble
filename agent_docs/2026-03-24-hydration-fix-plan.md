# Hydration & Reactivity Fix Plan

16 verified issues from code review (5 specialist agents). Ordered by dependency — later fixes depend on earlier ones.

## Principles

- Fix root causes, not symptoms
- Each fix must include a test update or new test
- No new dependencies unless unavoidable (no Pinia yet — use composables for shared state)
- KISS — smallest change that fixes the bug

---

## Phase A: Foundation Fixes (do first — other fixes depend on these)

### A1. `useToast()` must throw, not return `undefined`

**Root cause:** `useToast()` returns `ToastApi | undefined`, forcing optional chaining everywhere. If injection fails, ~20 error toasts silently no-op.

**Files:**
- `src/composables/toast.ts:12-14`
- Every view that uses `toast?.addToast` → change to `toast.addToast`

**Change:**
```typescript
// src/composables/toast.ts
export function useToast(): ToastApi {
  const api = inject(TOAST_KEY);
  if (!api) {
    throw new Error("useToast() called outside ToastProvider");
  }
  return api;
}
```

Then in every consumer file, remove the `?.` optional chaining:
- `src/views/DocumentReviewView.vue` — `toasts?.addToast` → `toasts.addToast` (lines ~197, 225)
- `src/views/ResearchView.vue` — `toasts?.addToast` → `toasts.addToast` (lines ~123, 142, 155)
- `src/views/TranslationView.vue` — `toast?.addToast` → `toast.addToast` (lines ~86, 114, 117)
- `src/views/SettingsView.vue` — `toast?.addToast` → `toast.addToast` (lines ~114, 121, 138, etc.)
- `src/views/DocumentDraftView.vue` — `toasts?.addToast` → `toasts.addToast` (lines ~77, 80, 88, etc.)
- `src/views/EvalsView.vue` — `toast?.addToast` → `toast.addToast` (lines ~102, 107, 123, etc.)

**Test:** Add to `tests/toast-provider.test.ts`:
```typescript
it("useToast throws when called outside provider", () => {
  const Orphan = defineComponent({
    setup() { useToast(); return {}; },
    template: "<div />",
  });
  expect(() => mount(Orphan)).toThrow("useToast() called outside ToastProvider");
});
```

### A2. Fix test files using wrong toast injection key

**Root cause:** 3 test files still provide toast via bare `"toast"` string key instead of `TOAST_KEY` (Symbol). Toast mock is never injected — assertions pass vacuously.

**Files:**
- `tests/research-view.test.ts:22` — `toast: { addToast: mockAddToast }` → `[TOAST_KEY as symbol]: { addToast: mockAddToast }`
- `tests/translation-view.test.ts:21` — same change
- `tests/document-draft-view.test.ts:15` — same change

Each file also needs: `import { TOAST_KEY } from "../src/composables/toast"`

**Verify:** After this fix, run `pnpm vitest run` — some tests may now FAIL because the mock is actually injected and assertions about toast calls may need updating. That's the point — these tests were giving false confidence.

---

## Phase B: Bug Fixes (independent of each other, do in any order after Phase A)

### B1. CommandPalette `NaN` from empty list modulo

**Root cause:** `0 % 0 = NaN` when `filteredCommands.value.length === 0`. Corrupts `highlightedIndex`.

**File:** `src/components/CommandPalette.vue:49-55`

**Change:** Add guard at top of ArrowDown/ArrowUp handlers:
```typescript
} else if (event.key === "ArrowDown") {
    event.preventDefault();
    if (filteredCommands.value.length === 0) return;
    highlightedIndex.value = (highlightedIndex.value + 1) % filteredCommands.value.length;
} else if (event.key === "ArrowUp") {
    event.preventDefault();
    if (filteredCommands.value.length === 0) return;
    highlightedIndex.value =
      (highlightedIndex.value - 1 + filteredCommands.value.length) % filteredCommands.value.length;
```

**Test:** Add to `tests/command-palette.test.ts`:
```typescript
it("arrow keys do not corrupt index when no matches", async () => {
  // Type something that matches nothing, then press ArrowDown
  // Verify highlightedIndex stays at 0, not NaN
});
```

### B2. `removeSecret` must fail visibly when keychain delete fails

**Root cause:** UI removes the secret even when `invoke("delete_api_key")` throws. Credential lingers in OS keychain.

**File:** `src/views/SettingsView.vue:141-151`

**Change:**
```typescript
async function removeSecret(id: string) {
  const secret = secrets.value.find((s) => s.id === id);
  if (secret) {
    try {
      await invoke("delete_api_key", { provider: secret.provider });
    } catch (e) {
      console.error("Failed to delete keychain entry:", e);
      toast.addToast("Could not remove credential from system keychain.", "error");
      return; // DO NOT remove from UI list
    }
  }
  secrets.value = secrets.value.filter((s) => s.id !== id);
}
```

**Test:** Add to `tests/settings-view.test.ts`:
```typescript
it("does not remove secret from UI when keychain delete fails", async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error("keychain locked"));
  // Click remove, verify secret still in list and error toast shown
});
```

### B3. `isSyncingBackend` shared boolean → counter

**Root cause:** Single boolean shared across concurrent file uploads. First to finish hides spinner for all.

**File:** `src/views/DocumentReviewView.vue:41`

**Change:**
```typescript
// Before:
const isSyncingBackend = ref(false);

// After:
const syncingCount = ref(0);
const isSyncingBackend = computed(() => syncingCount.value > 0);
```

In `queueInitialReview`:
```typescript
// Before:
isSyncingBackend.value = true;
// ...
finally { isSyncingBackend.value = false; }

// After:
syncingCount.value++;
// ...
finally { syncingCount.value--; }
```

Template references to `isSyncingBackend` stay the same (it's now a computed that returns boolean).

### B4. `isResearching` shared boolean → per-thread

**Root cause:** Single boolean blocks all threads when any thread has an async call in flight.

**File:** `src/views/ResearchView.vue`

**Change:** Move `isResearching` to per-thread state. Add `isResearching: boolean` to the `ResearchThread` type, then:
```typescript
// Remove: const isResearching = ref(false);
// In submitPrompt:
thread.isResearching = true;
// ...
finally { thread.isResearching = false; }

// In template, replace isResearching with activeThread?.isResearching
```

### B5. `isSending` shared boolean → per-session

**Root cause:** Same pattern as B4 but in DocumentReviewView.

**File:** `src/views/DocumentReviewView.vue`

**Change:** Add `isSending: boolean` to `ReviewSession` type. Remove the top-level `isSending` ref. In `askQuestion`:
```typescript
session.isSending = true;
// ...
finally { session.isSending = false; }
```

Template: `isSending` → `activeSession?.isSending`

### B6. Partial review failure leaves stale messages

**Root cause:** If `mockInitialReview` throws after `mockRegisterReview` succeeds, the "Starting initial review..." and "Preparing initial summary..." messages remain while status reverts to `"idle"`.

**File:** `src/views/DocumentReviewView.vue:174-201`

**Change:** In the `catch` block, clear the stale messages:
```typescript
catch (error) {
  console.error(error);
  const session = sessions.value[file.id];
  if (session) {
    session.reviewStatus = "idle";
    session.messages = []; // Clear stale progress messages
  }
  toasts.addToast("Failed to start initial review. Please try again.", "error");
}
```

### B7. Destructuring crash on empty array

**Root cause:** `const [summary] = await mockRegisterReview([...])` crashes if returned array is empty.

**File:** `src/views/DocumentReviewView.vue:177`

**Change:**
```typescript
const results = await mockRegisterReview([...]);
if (!results.length) {
  toasts.addToast("File could not be registered for review.", "error");
  return;
}
const summary = results[0];
```

### B8. Translation same-language bypass with auto-detect

**Root cause:** When `sourceLanguage === "auto"`, the same-language guard is skipped, allowing auto-detected Spanish → Spanish.

**File:** `src/views/TranslationView.vue:85`

**Change:** This is acceptable behavior — when source is "auto", the user doesn't know the source language, so the check can't fire. However, the mock backend should detect this and warn. No code change needed, but add a comment:
```typescript
// When source is "auto", we can't check for same-language — the backend
// should detect and warn if the detected source matches the target.
```

**Verdict:** Skip this fix — YAGNI for alpha.

### B9. Custom sync URL SSRF surface

**Root cause:** `testSync()` passes user-entered URL to backend without validation.

**File:** `src/views/SettingsView.vue:167-170`

**Change:** Validate URL format before sending:
```typescript
async function testSync() {
  const url = syncSettings.useCustom ? syncSettings.customServer : syncSettings.server;
  try {
    new URL(url); // throws if invalid
  } catch {
    toast.addToast("Invalid server URL.", "error");
    return;
  }
  if (!url.startsWith("https://")) {
    toast.addToast("Sync server must use HTTPS.", "error");
    return;
  }
  // ... proceed with test
}
```

### B10. EvalsView concurrent benchmark guard

**Root cause:** No early-return on `isRunning` in `runBenchmark()`.

**File:** `src/views/EvalsView.vue:100`

**Change:** Add guard at top:
```typescript
async function runBenchmark(name: string, prompts: string[]) {
  if (isRunning.value) return;
  // ... rest of function
}
```

### B11. EvalsView `selectedModels` stale closure in `.then()`

**Root cause:** `selectedModels` is a computed read inside `.then()` after async delay — reflects post-toggle state.

**File:** `src/views/EvalsView.vue:109-116`

**Change:** Capture the value before the async call:
```typescript
async function runBenchmark(name: string, prompts: string[]) {
  if (isRunning.value) return;
  const capturedModels = [...selectedModels.value]; // snapshot
  isRunning.value = true;
  // ... use capturedModels in .then() instead of selectedModels.value
}
```

---

## Phase C: Architecture Improvements (do after A and B)

### C1. TranslationView editor state not reactive to job changes

**Root cause:** `updateEditorFromJob` is called once at setup time. No watcher syncs editor when `activeJobId` changes.

**File:** `src/views/TranslationView.vue:66`

**Change:** Replace the one-shot call with a watcher:
```typescript
// Remove: updateEditorFromJob(activeJob.value ?? null);

// Add:
watch(activeJob, (job) => {
  updateEditorFromJob(job ?? null);
}, { immediate: true });
```

### C2. DocumentDraftView Enter key should submit form

**Root cause:** `<form @submit.prevent>` has no handler — Enter in form fields does nothing.

**File:** `src/views/DocumentDraftView.vue:126`

**Change:**
```html
<form class="grid gap-4 md:grid-cols-2" @submit.prevent="generateDraft">
```

### C3. CommandPalette keydown listener should be scoped to open state

**Root cause:** Global keydown listener fires on every keystroke even when palette is closed.

**File:** `src/components/CommandPalette.vue:66-72`

**Change:** Replace `onMounted`/`onBeforeUnmount` with a watcher:
```typescript
// Remove onMounted/onBeforeUnmount

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      window.addEventListener("keydown", onKeydown);
    } else {
      window.removeEventListener("keydown", onKeydown);
    }
  },
);

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
});
```

Note: Keep the AppShell's Cmd+K listener — that one needs to fire when palette is closed.

### C4. Extract shared model inventory (DRY)

**Root cause:** Model lists duplicated in ResearchView, TranslationView, SettingsView. Adding API key in Settings doesn't update model availability elsewhere.

**Files to create:**
- `src/composables/models.ts` — shared model inventory composable

```typescript
// src/composables/models.ts
import { ref, computed } from "vue";

export type ModelOption = {
  id: string;
  label: string;
  provider: string;
  available: boolean;
};

const models = ref<ModelOption[]>([
  { id: "elefant-local", label: "Ollama · Elefant Legal Blend", provider: "Local", available: true },
  { id: "gpt-4.1-mini", label: "GPT-4.1 mini", provider: "OpenAI", available: false },
  { id: "sonnet-3.5", label: "Claude 3.5 Sonnet", provider: "Anthropic", available: false },
]);

export function useModels() {
  const availableModels = computed(() => models.value.filter((m) => m.available));

  function setAvailability(providerId: string, available: boolean) {
    for (const m of models.value) {
      if (m.provider.toLowerCase() === providerId.toLowerCase()) {
        m.available = available;
      }
    }
  }

  return { models, availableModels, setAvailability };
}
```

**Files to modify:**
- `src/views/ResearchView.vue` — replace local `modelInventory` with `useModels()`
- `src/views/TranslationView.vue` — replace local `modelOptions` with `useModels()`
- `src/views/SettingsView.vue` — call `setAvailability()` when secrets are added/removed

### C5. Extract shared `generateId` and message types (DRY)

**Root cause:** `generateId()`, `formatTimestamp()`, and message type duplicated across 4+ files.

**File to create:** `src/utils/ids.ts`
```typescript
export function generateId(): string {
  return Math.random().toString(36).slice(2, 10);
}

export function formatTimestamp(date = new Date()): string {
  return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}
```

**File to create:** `src/types/chat.ts`
```typescript
export type ChatMessage = {
  id: string;
  role: "assistant" | "user";
  content: string;
  timestamp: string;
  citations?: string[];
};
```

**Files to modify:** Replace local definitions in DocumentReviewView, ResearchView, TranslationView, SettingsView with imports.

### C6. Extract user info (DRY)

**Root cause:** Username/team hardcoded in both SidebarNav and TopBar.

**File to create:** `src/composables/user.ts`
```typescript
import { ref } from "vue";

export const currentUser = ref({
  name: "CoastalTower238",
  team: "SilverEcho951",
  version: "Rumble v0.1.0a",
});
```

**Files to modify:**
- `src/modules/navigation/SidebarNav.vue:42-46` — import `currentUser`
- `src/modules/navigation/TopBar.vue:47-53` — import `currentUser`

---

## Phase D: State Persistence (optional — depends on product direction)

### D1. Add `<KeepAlive>` to AppShell

**Root cause:** All view state destroyed on route navigation. Sessions, threads, forms lost.

**File:** `src/layouts/AppShell.vue:80`

**Option 1 — KeepAlive (simplest):**
```html
<KeepAlive>
  <RouterView />
</KeepAlive>
```

Pros: Zero-effort state persistence. Cons: Memory usage grows — all views stay in memory.

**Option 2 — Selective KeepAlive:**
```html
<RouterView v-slot="{ Component }">
  <KeepAlive include="DocumentReviewView,ResearchView,TranslationView">
    <component :is="Component" />
  </KeepAlive>
</RouterView>
```

Requires adding `name` to each view's `defineOptions({ name: "DocumentReviewView" })`.

**Option 3 — Pinia stores (heaviest):**
Move session/thread/form state into Pinia stores. Views become stateless renderers. Most robust but most work.

**Recommendation:** Option 2 for alpha. Migrate to Pinia when the app grows.

---

## Dependency Order

```
Phase A (foundation — do first)
  A1: useToast throws
  A2: fix test toast keys
    ↓
Phase B (bug fixes — independent, any order)
  B1: CommandPalette NaN
  B2: removeSecret fail visibility
  B3: isSyncingBackend → counter
  B4: isResearching → per-thread
  B5: isSending → per-session
  B6: partial review stale messages
  B7: destructuring crash
  B9: sync URL validation
  B10: EvalsView concurrent guard
  B11: EvalsView stale closure
    ↓
Phase C (architecture — do after B)
  C1: TranslationView watcher
  C2: Draft form Enter key
  C3: CommandPalette scoped listener
  C4: shared model inventory
  C5: shared ids/types
  C6: shared user info
    ↓
Phase D (optional — product decision)
  D1: KeepAlive for state persistence
```

## Effort Estimate

| Phase | Fixes | Effort |
|-------|-------|--------|
| A — Foundation | 2 | ~30 min |
| B — Bug fixes | 10 | ~2 hr |
| C — Architecture | 6 | ~2 hr |
| D — Persistence | 1 | ~30 min (KeepAlive) or ~4 hr (Pinia) |
| **Total** | **19** | **~5 hr** (without Pinia) |

## Verification

After all fixes, run:
```bash
pnpm vitest run          # all 126+ tests pass
pnpm build               # TypeScript + Vite build clean
pnpm test:e2e            # Playwright e2e pass (if dev server running)
```
