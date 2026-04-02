# Rumble — Readiness Implementation Plan

How to implement each item from `2026-04-03-readiness-spec.md`. Ordered by dependency — later tasks depend on earlier ones.

---

## Phase 1: Sidecar Works For Real

### 1A. Default to Ollama mode (spec 1.5, 1.6)

- [x] **P1.1** In `src-tauri/src/lib.rs`, `run()` function (~line 529): verify `BackendMode::Ollama` is the default in `AppState`. **Confirmed** — line 529: `Mutex::new(BackendMode::Ollama)`.
- [x] **P1.2** In `resolve_url()` (~line 76): verify that non-cloud paths route to sidecar in Ollama mode. **Confirmed** — `/chats`, `/translate`, `/draft`, `/review`, `/research` are not in `CLOUD_ONLY_PREFIXES`.
- [x] **P1.3** Test: start sidecar manually (`pnpm dev:sidecar`), then `pnpm tauri dev`. Use Vue devtools or console to call `invoke("api_call", {method: "GET", path: "/health"})`. Verify it hits the sidecar, not the cloud.

### 1B. DB writes to app data dir (spec 1.4)

- [x] **P1.4** In `src-tauri/sidecar/services/db.py`, `init_db()` (~line 57): verify `data_dir` parameter is used when provided. **Confirmed** — `Path(data_dir) / "rumble.db"`.
- [x] **P1.5** In `src-tauri/src/lib.rs`, `spawn_sidecar()` (~line 392): verify `--data-dir` is passed with Tauri's `app.path().app_data_dir()`. **Confirmed** — line 381-396.
- [ ] **P1.6** Test: `pnpm tauri dev`, then check `~/Library/Application Support/com.elefant.rumble/` (macOS) for `rumble.db`. Verify no `rumble.db` in project root.

### 1C. PyInstaller binary builds (spec 1.7, 1.8)

- [x] **P1.7** In `src-tauri/sidecar/sidecar.spec` (~line 16): verify `hiddenimports` includes `routes.jobs`, `services.jobs`, `services.prompts`. **Confirmed** — all three present at lines 40, 44, 45.
- [ ] **P1.8** Run: `cd src-tauri/sidecar && uv run pyinstaller sidecar.spec --noconfirm`. Verify `dist/rumble-sidecar` binary exists.
- [ ] **P1.9** Run: `./dist/rumble-sidecar --port 9999 --data-dir /tmp/rumble-test`. Verify `GET http://127.0.0.1:9999/health` returns `{"status":"ok"}`.
- [ ] **P1.10** Copy binary to `src-tauri/binaries/rumble-sidecar-$(rustc -vV | grep host | cut -d' ' -f2)`. Run `pnpm tauri dev`. Verify the app starts with the real binary (no separate terminal needed).

### 1D. Full dev-mode smoke test (spec 1.1–1.3, 1.9)

- [ ] **P1.11** Start Ollama with at least one model (`ollama pull llama3.2`).
- [x] **P1.12** `pnpm dev:sidecar` → verify `/health`, `/ready`, `/models` all return correct responses.
- [ ] **P1.13** `pnpm tauri dev` → verify app window opens, no white screen, lands on Document Review.

---

## Phase 2: AI Pipeline Doesn't Crash

### 2A. Fix _parse_llm_json (spec 2.5–2.9, 2.16)

- [x] **P2.1** Rewrite `_parse_llm_json()` in `src-tauri/sidecar/routes/ai.py` (~line 28). New logic:
  ```python
  def _parse_llm_json(raw: str) -> dict:
      text = raw.strip()
      # Strip markdown fence if present
      if "```" in text:
          # Find the JSON block between fences
          parts = text.split("```")
          for part in parts:
              candidate = part.strip()
              # Remove optional language tag (e.g. "json\n")
              if candidate.startswith(("json", "JSON")):
                  candidate = candidate.split("\n", 1)[-1].strip()
              if candidate.startswith("{"):
                  text = candidate
                  break
      try:
          return json.loads(text)
      except json.JSONDecodeError as e:
          raise HTTPException(502, f"LLM returned unparseable output: {e}")
  ```
- [x] **P2.2** Add unit tests in `src-tauri/sidecar/tests/test_parse_llm_json.py`:
  - Raw JSON → parses
  - Fenced with `json` tag → parses
  - Fenced without tag → parses
  - Trailing newline after fence → parses
  - Preamble text before fence → parses
  - Garbage → returns 502 with clear message
- [x] **P2.3** Apply same fix to `_parse_job_json()` in `routes/jobs.py` (~line 21) — refactored to delegate to `_parse_llm_json`, converting HTTPException to ValueError.

### 2B. Fix multi-turn chat context (spec 2.1–2.4)

- [x] **P2.4** In `src-tauri/sidecar/services/llm.py`, `_to_message_history()` (~line 33): the function already excludes the last message (`messages[:-1]`) because `send_message()` passes `messages[-1]` as `user_text` separately. **This is correct by design** — PydanticAI's `agent.run(user_text, message_history=...)` sends the user text as the new prompt, and history as prior context. Verify this is working:
- [x] **P2.5** Test: create a chat, send "My name is Alice", then send "What is my name?". Verify the response references "Alice". If it doesn't, the issue is in how `chat.py` builds the messages list.
- [x] **P2.6** In `routes/chat.py` `send_message()` (~line 65): verify `db.get_messages(chat_id)` returns all messages including the just-added user message. Trace: line 79 adds user msg → line 82 fetches all messages → line 84 calls `llm.send_message(messages)`. The full history should be there.
- [x] **P2.7** If the test in P2.5 fails: add debug logging to `_to_message_history()` to see what's actually sent to the LLM. Fix accordingly.

### 2C. AI endpoints work with real Ollama (spec 2.10–2.15)

- [x] **P2.8** For each endpoint, run a manual curl test against the running sidecar:
  ```bash
  # Clarify
  curl -X POST http://127.0.0.1:8000/clarify \
    -H 'Content-Type: application/json' \
    -d '{"ask": "What is a force majeure clause?"}'

  # Translate
  curl -X POST http://127.0.0.1:8000/translate \
    -H 'Content-Type: application/json' \
    -d '{"text": "This agreement is binding.", "target_lang": "fr"}'

  # Summarise document
  curl -X POST http://127.0.0.1:8000/summarise/document \
    -H 'Content-Type: application/json' \
    -d '{"text": "This is a sample employment agreement between Company A and Employee B. The term of employment shall be for a period of two years commencing on January 1, 2026."}'

  # Chat title
  curl -X POST http://127.0.0.1:8000/chat_title/generate \
    -H 'Content-Type: application/json' \
    -d '{"messages": [{"role": "user", "content": "What is a force majeure clause?"}]}'
  ```
- [x] **P2.9** Fix any endpoint that returns 500/502. Most likely cause: `_parse_llm_json` (fixed in P2.1) or `_safe_construct` missing required fields. For missing fields, add defaults to the `_safe_construct` call.

### 2D. Job endpoints work (spec 2.17–2.20)

- [x] **P2.10** Test draft job: verified with correct payload (`prompt` + `parties` as list[dict]).
- [x] **P2.11** Test review job: verified with `text` field. Returns summary + issues.
- [x] **P2.12** Test research job: verified. Required `_normalize_sources()` fix for SearchResult schema.
- [ ] **P2.13** Verify failed jobs: kill Ollama mid-request. Check that job status becomes "failed" with error message, not stuck at "running" forever.

---

## Phase 3: Views Use Real Sidecar

### 3A. Create shared API helper (foundation for all views)

- [ ] **P3.1** Create `src/api/sidecar.ts` — thin wrappers around `invoke("api_call", ...)` for each sidecar endpoint:
  ```typescript
  // All calls go through Tauri IPC → Rust api_call → sidecar HTTP
  export async function createChat(title?: string) { ... }
  export async function sendMessage(chatId: string, text: string) { ... }
  export async function streamMessage(chatId: string, text: string) { ... }
  export async function createDraftJob(payload: DraftPayload) { ... }
  export async function pollJobResult(type: string, jobId: string) { ... }
  export async function createReviewJob(payload: ReviewPayload) { ... }
  export async function createResearchJob(payload: ResearchPayload) { ... }
  export async function translate(payload: TranslatePayload) { ... }
  export async function clarify(payload: ClarifyPayload) { ... }
  ```
  Each function: calls `apiClient()` (from `src/api/client.ts`), types the response, throws on error.

### 3B. Wire Document Review (spec 3.1–3.7)

- [ ] **P3.2** In `src/views/DocumentReviewView.vue`:
  - Replace `mockRegisterReview()` call (~line 166) with `createReviewJob()` from sidecar API.
  - Replace `mockInitialReview()` (~line 182) with `pollJobResult("review", jobId)` — poll until complete.
  - Replace `mockDocumentChat()` (~line 217) with `sendMessage(chatId, question)`.
- [ ] **P3.3** Add loading state: set `isSending = true` before `sendMessage`, reset in finally block. Bind `:disabled="isSending"` on the Send button.
- [ ] **P3.4** Add error handling: wrap each call in try/catch. On error: show toast, reset UI state (remove dangling user message if response failed, reset review status if initial review failed).
- [ ] **P3.5** Test: upload a file → verify review appears → ask a follow-up → verify response → verify errors show toast.

### 3C. Wire Research (spec 3.8–3.12)

- [ ] **P3.6** In `src/views/ResearchView.vue`:
  - Replace `mockResearchRun()` call (~line 130) with `createResearchJob()` + poll.
  - Add `isResearching` ref. Set true before request, false in finally.
  - Bind `:disabled="isResearching"` on "Start Research" button.
- [ ] **P3.7** Add error handling: on failure, reset thread status to "draft", show toast.
- [ ] **P3.8** Test: submit a research prompt → verify result with citations → verify error handling.

### 3D. Wire Document Draft (spec 3.13–3.17)

- [ ] **P3.9** In `src/views/DocumentDraftView.vue`:
  - Replace mock setTimeout (~line 76) with `createDraftJob()` + poll.
  - `isGenerating` already exists — ensure it resets on error path too.
- [ ] **P3.10** Add error handling: wrap in try/catch, show toast, reset `isGenerating`.
- [ ] **P3.11** Test: select template, fill fields, generate → verify draft appears → verify errors.

### 3E. Wire Translation (spec 3.18–3.22)

- [ ] **P3.12** In `src/views/TranslationView.vue`:
  - Replace `mockTranslationRun()` call (~line 88) with `translate()` from sidecar API.
  - `isTranslating` already exists and disables the button.
- [ ] **P3.13** Add error handling: wrap in try/catch, show toast with message.
- [ ] **P3.14** Test: enter text, select languages, translate → verify result → verify errors.

### 3F. Wire Settings (spec 3.23–3.25)

- [ ] **P3.15** In `src/views/SettingsView.vue`:
  - Replace mock save with `invoke("store_api_key", {provider, key})` for each secret.
  - Add `onMounted` hook that loads existing keys via `invoke("get_api_key", {provider})` for each provider.
  - Wire delete button to `invoke("delete_api_key", {provider})`.
- [ ] **P3.16** Test: add an OpenAI key → reload page → key still there → delete → gone.

---

## Bug Fixes

> When a bug is found during implementation (logged in spec Bugs Encountered), add the fix task here.

| # | Bug (from spec) | Fix Description | File(s) | Status |
|---|-----------------|-----------------|---------|--------|
| B1 | Default model not found | Auto-resolve Ollama model: query `/api/tags`, use first available if default missing | `services/llm.py` | Done |
| B2 | `<think>` tags break JSON parse | Strip `<think>...</think>` with regex in `_parse_llm_json` | `routes/ai.py`, `tests/test_parse_llm_json.py` | Done |
| B3 | Research sources schema mismatch | `_normalize_sources()` converts strings/partial dicts to SearchResult-compatible dicts | `routes/jobs.py` | Done |

---

## Verification

After all phases complete, the full smoke test is:

1. Start Ollama with a pulled model
2. `pnpm tauri dev` (sidecar starts automatically via Tauri)
3. App opens → Document Review page
4. Upload a text file → review appears
5. Ask a follow-up question → response appears
6. Navigate to Research → submit prompt → result with citations
7. Navigate to Draft → select template → fill fields → generate → draft appears
8. Navigate to Translation → enter text → translate → result appears
9. Navigate to Settings → add/remove API key → persists across reload
10. Kill Ollama → try an action → friendly error toast (not broken UI)
