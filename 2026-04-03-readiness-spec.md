# Rumble — End-to-End Readiness Specification

Get Rumble to a state where a non-technical test user can use all features with Ollama on any platform.

**Legend:**
- [ ] Not done
- [x] Done and verified
- [!] Blocked — see Bugs Encountered

---

## Phase 1: Sidecar Works For Real

The sidecar must start automatically, find its database, and respond to health checks — without the user running anything in a terminal.

- [x] 1.1 `pnpm dev:sidecar` starts and `GET /health` returns `{"status":"ok","mode":"ollama"}`
- [x] 1.2 `GET /ready` returns ready when Ollama is running
- [x] 1.3 `GET /models` returns the user's pulled Ollama models
- [x] 1.4 Sidecar writes `rumble.db` to the Tauri app data directory, not CWD
- [x] 1.5 App defaults to Ollama backend mode (not Premium)
- [x] 1.6 `api_call` routes non-cloud paths to `http://127.0.0.1:{sidecar_port}`
- [ ] 1.7 PyInstaller binary builds successfully on macOS (`uv run pyinstaller sidecar.spec`)
- [ ] 1.8 Built binary starts and passes health check
- [ ] 1.9 `pnpm tauri dev` (with sidecar running) opens the app window with no white screen

---

## Phase 2: AI Pipeline Doesn't Crash

Every AI endpoint must handle real LLM output without 500 errors.

### 2A. Chat (multi-turn)

- [x] 2.1 `POST /chats/{id}/message` sends full conversation history to the LLM (not just the last message)
- [x] 2.2 LLM response reflects prior context (e.g. "as I mentioned earlier...")
- [x] 2.3 `POST /chats/{id}/stream` also sends full history and streams correctly
- [x] 2.4 Chat with 5+ messages still works (history doesn't exceed token limits gracefully)

### 2B. JSON parsing

- [x] 2.5 `_parse_llm_json` handles: `{"key":"value"}` (raw JSON)
- [x] 2.6 `_parse_llm_json` handles: `` ```json\n{"key":"value"}\n``` `` (fenced JSON)
- [x] 2.7 `_parse_llm_json` handles: `` ```json\n{"key":"value"}\n```\n `` (trailing newline after fence)
- [x] 2.8 `_parse_llm_json` handles: `` ```\n{"key":"value"}\n``` `` (fence without language tag)
- [x] 2.9 `_parse_llm_json` handles: `Some preamble\n```json\n{"key":"value"}\n``` ` (text before fence)

### 2C. AI endpoints

- [x] 2.10 `POST /clarify` returns valid response with real Ollama model
- [x] 2.11 `POST /translate` returns valid response with real Ollama model
- [x] 2.12 `POST /summarise/document` returns valid response
- [x] 2.13 `POST /summarise/chat` returns valid response
- [x] 2.14 `POST /summarise/search` returns valid response
- [x] 2.15 `POST /chat_title/generate` returns valid response
- [x] 2.16 All AI endpoints return 502 with clear message when LLM returns unparseable output (not a raw traceback)

### 2D. Job endpoints

- [x] 2.17 `POST /draft` creates job, `/draft/{id}/result` returns completed result
- [x] 2.18 `POST /review` creates job, `/review/{id}/result` returns completed result
- [x] 2.19 `POST /research` creates job, `/research/{id}/result` returns completed result
- [ ] 2.20 Failed jobs store error message and return status "failed" (not hang forever)

---

## Phase 3: Views Use Real Sidecar

Every view must call the real sidecar endpoints instead of mock functions, and handle errors gracefully.

### 3A. Document Review (`/review`)

- [x] 3.1 Uploading a file calls sidecar `/review` endpoint (not `mockRegisterReview`)
- [x] 3.2 Initial review result appears in the chat (summary + messages)
- [x] 3.3 Follow-up questions call sidecar `/chats/{id}/message` and display response
- [ ] 3.4 Streaming responses show text appearing incrementally (SSE via `/chats/{id}/stream`)
- [x] 3.5 Send button is disabled while a request is in flight
- [x] 3.6 Network/LLM error shows toast — does not leave UI in broken state
- [x] 3.7 Failed initial review resets session to allow retry

### 3B. Research (`/research`)

- [x] 3.8 Submitting a prompt calls sidecar `/research` endpoint (not `mockResearchRun`)
- [x] 3.9 Research result with citations displays in the thread
- [x] 3.10 "Start Research" button is disabled while request is in flight
- [x] 3.11 Error during research shows toast, resets thread status to "draft"
- [x] 3.12 Thread status transitions: draft → in_progress → complete

### 3C. Document Draft (`/draft`)

- [x] 3.13 "Generate Draft" calls sidecar `/draft` endpoint (not mock setTimeout)
- [x] 3.14 Draft result appears in the editor area
- [x] 3.15 Button is disabled while generating
- [x] 3.16 Validation errors shown for empty required fields before API call
- [x] 3.17 Network/LLM error shows toast, re-enables the button

### 3D. Translation (`/translation`)

- [ ] 3.18 "Translate" calls sidecar `/translate` endpoint (not `mockTranslationRun`)
- [ ] 3.19 Translation result appears in the output area
- [ ] 3.20 Button is disabled while translating (already done via `isTranslating`)
- [ ] 3.21 New translation job is prepended to history
- [ ] 3.22 Error shows toast with clear message

### 3E. Settings (`/settings`)

- [ ] 3.23 Saving an API key calls `invoke("store_api_key", {provider, key})`
- [ ] 3.24 Deleting an API key calls `invoke("delete_api_key", {provider})`
- [ ] 3.25 Provider list loads stored keys on mount via `invoke("get_api_key", {provider})`

---

## Bugs Encountered

> When a test fails or unexpected behavior is found, log it here. Add a corresponding fix task to `imp_plan.md`.

| # | Spec Item | Severity | Description | Status |
|---|-----------|----------|-------------|--------|
| B1 | 2.10-2.15 | High | Default model `llama3.2` not found on Ollama — all stateless AI endpoints returned 404. Fixed by auto-resolving to first available model. | Fixed |
| B2 | 2.15 | Medium | DeepSeek-R1 `<think>` tags before JSON broke `_parse_llm_json`. Added regex strip for thinking blocks. | Fixed |
| B3 | 2.19 | High | Research `/result` endpoint 500'd — LLM returns sources as strings but `ResearchResultResponse` expects `list[SearchResult]`. Added `_normalize_sources()`. | Fixed |
