# Rumble Test Checklist — Every Interaction & Function

## Status Legend
- [x] Already tested
- [ ] Missing — needs test

---

## 1. Python Sidecar — Integration Tests (HTTP endpoint round-trips)

### 1.1 Health Endpoints (`routes/health.py`)

| # | Endpoint | Test | Status |
|---|----------|------|--------|
| 1 | `GET /health` | Returns 200 + `{status: ok, mode}` | [x] `test_health_returns_ok` |
| 2 | `GET /health` | Mode defaults to `ollama` when `RUMBLE_BACKEND_MODE` is missing | [x] `test_health_mode_defaults_to_ollama` |
| 3 | `GET /health` | Mode is `byok` when `RUMBLE_BACKEND_MODE=byok` (a key alone never selects it) | [x] `test_health_mode_byok_when_mode_env_set` |
| 4 | `GET /ready` | BYOK always ready | [x] `test_ready_byok_always_ready` |
| 5 | `GET /ready` | Ollama unreachable reports not_ready | [x] `test_ready_ollama_reports_unreachable` |
| 6 | `GET /ready` | Ollama mode with a reachable loopback fake Ollama → ready | [x] `test_ready_ollama_ready_when_reachable` |
| 7 | `GET /models` | BYOK returns static model list with default flag | [x] `test_models_byok_returns_static_list` |
| 8 | `GET /models` | Ollama returns list or empty | [x] `test_models_ollama_returns_list_or_empty` |
| 9 | `GET /models` | Ollama with a fake tags response → parsed models, cloud/remote ones hidden | [x] `test_models_ollama_filters_cloud_models` |

### 1.2 Chat Endpoints (`routes/chat.py`)

| # | Endpoint | Test | Status |
|---|----------|------|--------|
| 10 | `POST /chats` | Create with title → 200 + id | [x] `test_create_chat` |
| 11 | `POST /chats` | Create without body → 200 + id | [x] `test_create_chat_without_body` |
| 12 | `GET /chats` | Empty list → `{chats: []}` | [x] `test_list_chats_empty` |
| 13 | `GET /chats` | After 2 creates → 2 chats | [x] `test_list_chats_after_create` |
| 14 | `GET /chats/{id}` | Existing chat → 200 + messages array | [x] `test_get_chat_with_messages` |
| 15 | `GET /chats/{id}` | Nonexistent → 404 | [x] `test_get_nonexistent_chat` |
| 16 | `DELETE /chats/{id}` | Existing → 200 + `{status: ok}` + verify gone | [x] `test_delete_chat` |
| 17 | `DELETE /chats/{id}` | Nonexistent → 404 | [x] `test_delete_nonexistent_chat` |
| 18 | `PATCH /chats/{id}` | Update title → 200 + new title | [x] `test_update_chat_title` |
| 19 | `PATCH /chats/{id}` | Update nonexistent → 404 | [ ] |
| 20 | `GET /chats/{id}/messages` | Empty chat → `{messages: []}` | [x] `test_get_messages_empty_chat` |
| 21 | `GET /chats/{id}/messages` | Nonexistent → 404 | [x] `test_get_messages_nonexistent_chat` |
| 22 | `POST /chats/{id}/message` | Valid → 200 + assistant reply | [x] `test_send_message_returns_assistant_reply` |
| 23 | `POST /chats/{id}/message` | Stores user + assistant messages | [x] `test_send_message_stores_both_messages` |
| 24 | `POST /chats/{id}/message` | Empty text → 422 | [x] `test_send_message_missing_text` |
| 25 | `POST /chats/{id}/message` | Nonexistent chat → 404 | [x] `test_send_message_nonexistent_chat` |
| 26 | `POST /chats/{id}/message` | Multiple messages accumulate | [x] `test_conversation_accumulates` |
| 27 | `POST /chats/{id}/message` | With `model` field → forwarded to LLM | [ ] |
| 28 | `POST /chats/{id}/stream` | Valid → 200 + SSE events (status, delta, done) | [x] `test_stream_message` |
| 29 | `POST /chats/{id}/stream` | Persists user + assistant messages | [x] `test_stream_message_persists_messages` |
| 30 | `POST /chats/{id}/stream` | Nonexistent chat → 404 | [x] `test_stream_nonexistent_chat` |
| 31 | `POST /chats/{id}/stream` | Empty text → 422 | [x] `test_stream_missing_text` |

### 1.3 AI Endpoints (`routes/ai.py`)

| # | Endpoint | Test | Status |
|---|----------|------|--------|
| 32 | `POST /clarify` | Valid ask → structured response | [x] `test_clarify_returns_structured_response` |
| 33 | `POST /clarify` | With context, document_type → 200 | [x] `test_clarify_with_context` |
| 34 | `POST /clarify` | Empty ask → 422 | [x] `test_clarify_empty_ask_rejected` |
| 35 | `POST /clarify` | Missing `ask` field entirely → 422 | [ ] |
| 36 | `POST /clarify` | With all optional fields (parties, terms, refs) | [ ] |
| 37 | `POST /chat_title/generate` | Valid messages → title | [x] `test_chat_title_generate` |
| 38 | `POST /chat_title/generate` | Empty messages → 422 | [x] `test_chat_title_empty_messages_rejected` |
| 39 | `POST /translate` | Valid → translated_text | [x] `test_translate_returns_translated_text` |
| 40 | `POST /translate` | Missing target_lang → 422 | [x] `test_translate_missing_target_lang_rejected` |
| 41 | `POST /translate` | Empty text → 422 | [ ] |
| 42 | `POST /summarise/document` | Valid → summary + key_points | [x] `test_summarise_document` |
| 43 | `POST /summarise/document` | With style → 200 | [ ] |
| 44 | `POST /summarise/document` | Empty text → 422 | [ ] |
| 45 | `POST /summarise/chat` | Valid messages → summary | [x] `test_summarise_chat` |
| 46 | `POST /summarise/chat` | With style → 200 | [ ] |
| 47 | `POST /summarise/search` | Valid → summary + citations | [x] `test_summarise_search` |
| 48 | `POST /summarise/search` | Empty results → 422 | [ ] |
| 49 | (internal) | `_parse_llm_json` strips markdown fences | [ ] |
| 50 | (internal) | `_parse_llm_json` invalid JSON → HTTPException 502 | [ ] |

### 1.4 Job Endpoints (`routes/jobs.py`)

| # | Endpoint | Test | Status |
|---|----------|------|--------|
| 51 | `POST /draft` | Creates job → `{job_id, status: queued, poll_url}` | [x] `test_draft_creates_job` |
| 52 | `GET /draft/{id}/result` | Completed job → result data | [x] `test_draft_completes` |
| 53 | `GET /draft/{id}/result` | Nonexistent → 404 | [x] `test_draft_nonexistent_job` |
| 54 | `POST /draft` | With all optional fields (document_type, style, parties, terms) | [ ] |
| 55 | `POST /review` | Creates job | [x] `test_review_creates_job` |
| 56 | `GET /review/{id}/result` | Completed → result | [x] `test_review_completes` |
| 57 | `GET /review/{id}/result` | Nonexistent → 404 | [ ] |
| 58 | `POST /research` | Creates job → `{report_id, status: queued}` | [x] `test_research_creates_job` |
| 59 | `GET /research/{id}` | Status check → metadata | [x] `test_research_status` |
| 60 | `GET /research/{id}/result` | Completed → result + sources | [x] `test_research_result` |
| 61 | `GET /research/{id}` | Nonexistent → 404 | [x] `test_research_nonexistent_job` |
| 62 | `GET /research/{id}/result` | Nonexistent → 404 | [ ] |
| 63 | `POST /research` | With constraints, guidelines, primary_source | [ ] |
| 64 | (lifecycle) | Job transitions: queued → running → completed | [ ] |
| 65 | (lifecycle) | Job failure: error stored, status = failed | [ ] |

### 1.5 Database Service (`services/db.py`)

| # | Function | Test | Status |
|---|----------|------|--------|
| 66 | `init_db` | Creates tables and indices | [x] (implicit in fixtures) |
| 67 | `close_db` | Closes connection | [x] (implicit in fixtures) |
| 68 | `_get_db()` | Raises RuntimeError before init | [ ] |
| 69 | `create_chat(title)` | Returns id, title, created_at | [x] `test_create_chat_returns_id` |
| 70 | `create_chat()` | No title → title is None | [x] `test_create_chat_without_title` |
| 71 | `list_chats` | Ordered newest first | [x] `test_list_chats_ordered_newest_first` |
| 72 | `get_chat` | Found → dict | [x] `test_get_chat_found` |
| 73 | `get_chat` | Missing → None | [x] `test_get_chat_missing_returns_none` |
| 74 | `delete_chat` | Found → True, gone | [x] `test_delete_chat` |
| 75 | `delete_chat` | Missing → False | [x] `test_delete_nonexistent_chat` |
| 76 | `update_chat` | Title updated | [x] `test_update_chat_title` |
| 77 | `update_chat` | None title preserves existing | [x] `test_update_chat_preserves_title_when_none` |
| 78 | `add_message` | Returns fields | [x] `test_message_fields` |
| 79 | `get_messages` | Ordered by created_at | [x] `test_messages_ordered_by_created_at` |
| 80 | `add_message` | Updates chat timestamp | [x] `test_add_message_updates_chat_timestamp` |
| 81 | `delete_chat` | Cascades to messages | [x] `test_delete_chat_cascades_to_messages` |
| 82 | `get_messages` | Empty chat → [] | [x] `test_get_messages_for_empty_chat` |
| 83 | `create_job` | Returns id, type, status, created_at | [ ] |
| 84 | `get_job` | Found → dict with all fields | [ ] |
| 85 | `get_job` | Missing → None | [ ] |
| 86 | `update_job_status(running)` | Sets started_at | [ ] |
| 87 | `update_job_status(completed)` | Sets completed_at | [ ] |
| 88 | `set_job_result(result=...)` | Sets status=completed, stores result | [ ] |
| 89 | `set_job_result(error=...)` | Sets status=failed, stores error | [ ] |

### 1.6 LLM Service (`services/llm.py`) — unit-testable with mocks

| # | Function | Test | Status |
|---|----------|------|--------|
| 90 | `_build_agent(byok)` | `RUMBLE_BACKEND_MODE=byok` → OpenAI provider with the key at the pinned public base_url | [x] `test_build_agent_byok_mode_uses_key_no_ollama_call` |
| 91 | `_build_agent(ollama)` | Ollama mode, even with a key present → Ollama provider at the loopback base_url | [x] `test_build_agent_ollama_mode_ignores_byok_env_key` |
| 92 | `send_message` | Returns full text | [x] (tested via route integration) |
| 93 | `stream_message` | Yields text chunks | [x] (tested via route integration) |
| 94 | `run_single_turn` | Returns raw text | [x] (tested via route integration) |

### 1.7 Jobs Service (`services/jobs.py`)

| # | Function | Test | Status |
|---|----------|------|--------|
| 95 | `start_job` | Spawns background task | [x] (tested via route integration) |
| 96 | `_run_job` | Success → completed + result stored | [ ] |
| 97 | `_run_job` | Exception → failed + error stored | [ ] |

---

## 2. TypeScript/Vue — Vitest Tests

### 2.1 Pure Utility Functions (`src/utils.ts`)

| # | Function | Test | Status |
|---|----------|------|--------|
| 98 | `formatSize(0)` | "0.0 B" | [x] |
| 99 | `formatSize(512)` | "512.0 B" | [x] |
| 100 | `formatSize(1024)` | "1.0 KB" | [x] |
| 101 | `formatSize(1536)` | "1.5 KB" | [x] |
| 102 | `formatSize(MB/GB/TB)` | Correct units | [x] |
| 103 | `formatSize(>TB)` | Caps at TB | [x] |
| 104 | `oldestFileDate([])` | "—" | [x] |
| 105 | `oldestFileDate(single)` | Date string | [x] |
| 106 | `oldestFileDate(multi)` | Oldest date | [x] |
| 107 | `largestFile([])` | null | [x] |
| 108 | `largestFile(single)` | That file | [x] |
| 109 | `largestFile(multi)` | Largest | [x] |
| 110 | `largestFile(tie)` | First | [x] |

### 2.2 API Client (`src/api/client.ts`)

| # | Interaction | Test | Status |
|---|-------------|------|--------|
| 111 | GET with path only | invoke(api_call, {method: GET, path, body: null, params: null}) | [x] |
| 112 | Default method → GET | Omitted method defaults to GET | [x] |
| 113 | Query param extraction | `?q=x&page=2` → params object | [x] |
| 114 | JSON string body parsing | JSON.stringify(obj) → parsed object | [x] |
| 115 | POST method forwarding | method: POST | [x] |
| 116 | DELETE method forwarding | method: DELETE | [x] |
| 117 | Returns invoke result | Pass-through | [x] |
| 118 | Propagates invoke errors | Rejects with error | [x] |
| 119 | URL with no query params | params: null | [x] |
| 120 | Null body | body: null | [x] |
| 121 | PUT method forwarding | method: PUT | [ ] |
| 122 | PATCH method forwarding | method: PATCH | [ ] |
| 123 | Non-string body (object) | Passed through without JSON.parse | [ ] |

### 2.3 CommandPalette Component

| # | Interaction | Test | Status |
|---|-------------|------|--------|
| 124 | Renders all commands when open | 4 items | [x] |
| 125 | Hidden when closed | No input rendered | [x] |
| 126 | Filters by query (case-insensitive) | "doc" → 2 items | [x] |
| 127 | No-matches message | "zzz" → "No matches" | [x] |
| 128 | First item highlighted by default | bg-primary class | [x] |
| 129 | ArrowDown moves highlight | index 0→1 | [x] |
| 130 | ArrowDown wraps to first | last→first | [x] |
| 131 | ArrowUp wraps to last | first→last | [x] |
| 132 | Enter executes + closes | action called + close emitted | [x] |
| 133 | Escape closes | close emitted | [x] |
| 134 | Click executes + closes | action called + close emitted | [x] |
| 135 | Mouse enter highlights | hover index | [x] |
| 136 | Shortcut text displayed | "Ctrl+T" visible | [x] |

### 2.4 ToastProvider Component

| # | Interaction | Test | Status |
|---|-------------|------|--------|
| 137 | Renders slot content | Button exists | [x] |
| 138 | No toasts initially | Empty | [x] |
| 139 | addToast shows toast | Text appears | [x] |
| 140 | Multiple toasts | 3 elements | [x] |
| 141 | Auto-removes after 3000ms | Gone after timer advance | [x] |
| 142 | Success type → bg-success class | CSS class check | [x] |
| 143 | Error type → bg-error class | CSS class check | [x] |
| 144 | Custom duration | Respects non-default duration | [ ] |
| 145 | Info type → correct class | CSS class for info | [ ] |

### 2.5 SidebarNav Component

| # | Interaction | Test | Status |
|---|-------------|------|--------|
| 146 | Renders all reorderable tools | 5 tool names present | [x] |
| 147 | Renders fixed tools (Settings) | "Settings" present | [x] |
| 148 | Emits navigate on tool click | navigate + path | [x] |
| 149 | No navigate for disabled tools | aria-disabled → no emit | [x] |
| 150 | Move tool down | ArrowDown → order changed | [x] |
| 151 | Move tool up | ArrowUp → order changed | [x] |
| 152 | Disable move-up for first | disabled attribute | [x] |
| 153 | Disable move-down for last | disabled attribute | [x] |
| 154 | Coming Soon sublabel | Text present | [x] |
| 155 | Version info | "Rumble v0.1.0a" | [x] |
| 156 | Active path highlighting | activePath prop → highlight | [ ] |

### 2.6 DocumentDraftView Component

| # | Interaction | Test | Status |
|---|-------------|------|--------|
| 157 | Renders page title | "Document Draft" | [x] |
| 158 | Renders template library | 3 template names | [x] |
| 159 | Validation error on empty blur | "is required" | [x] |
| 160 | Clears error when filled | Error disappears | [x] |
| 161 | All errors on submit | All "required" errors | [x] |
| 162 | No errors with filled fields | Clean submit | [x] |
| 163 | Template selection | Active class on click | [x] |
| 164 | Export buttons | "Export to Word" + "Export to PDF" | [x] |

### 2.7 LoginView Component — NOT TESTED

| # | Interaction | Test | Status |
|---|-------------|------|--------|
| 165 | Renders login form | Passphrase input, button, checkbox | [ ] |
| 166 | Shows error on empty passphrase submit | "Passphrase required" | [ ] |
| 167 | Clears error on valid input + submit | Error disappears | [ ] |
| 168 | Shows loading state during submission | "Authenticating..." visible | [ ] |
| 169 | Navigates to /review on success | router.push called | [ ] |
| 170 | "Remember for 30 days" checkbox | v-model binding | [ ] |
| 171 | "Generate phrase" button renders | Button text present | [ ] |

### 2.8 SettingsView Component — NOT TESTED

| # | Interaction | Test | Status |
|---|-------------|------|--------|
| 172 | Renders page title and tabs | "Settings" + 4 tab buttons | [ ] |
| 173 | Tab switching | Click tab → active class + content changes | [ ] |
| 174 | Providers tab: lists existing secrets | Secret label visible | [ ] |
| 175 | Add secret: validates provider required | No provider → no add | [ ] |
| 176 | Add secret: validates key for requiresKey providers | Error toast | [ ] |
| 177 | Add secret: successful add | New entry appears in list | [ ] |
| 178 | Remove secret | Click Remove → entry gone | [ ] |
| 179 | Local config fields | host, port, model inputs | [ ] |
| 180 | Save settings: loading state | "Saving..." visible | [ ] |
| 181 | Storage tab: template path + workspace path | Input fields visible | [ ] |
| 182 | Appearance tab: theme/sidebar selects | Select options present | [ ] |
| 183 | Sync tab: enable toggle + team code | Checkbox + input | [ ] |
| 184 | Sync tab: test connection | Toast on click | [ ] |
| 185 | Sync tab: custom server toggle | Enables/disables custom URL input | [ ] |

### 2.9 ResearchView Component — NOT TESTED

| # | Interaction | Test | Status |
|---|-------------|------|--------|
| 186 | Renders page title | "Research Assistant" | [ ] |
| 187 | Renders initial threads in sidebar | Thread titles visible | [ ] |
| 188 | New Thread button creates thread | Thread count increases | [ ] |
| 189 | Thread activation | Click → active class | [ ] |
| 190 | Submit prompt: adds user message | Message appears in chat | [ ] |
| 191 | Submit prompt: empty → no action | No message added | [ ] |
| 192 | Submit prompt: shows research response | Assistant message appears | [ ] |
| 193 | Thread status changes | draft → running → complete | [ ] |
| 194 | Model selector | Available models selectable | [ ] |
| 195 | Workflow notes rendered | 3 workflow tips | [ ] |

### 2.10 TranslationView Component — NOT TESTED

| # | Interaction | Test | Status |
|---|-------------|------|--------|
| 196 | Renders page title | "Translation" | [ ] |
| 197 | Renders language selectors | Source + Target dropdowns | [ ] |
| 198 | Renders job history | Initial job visible | [ ] |
| 199 | Translation execution | Source text → translated text appears | [ ] |
| 200 | Empty source → no action | Button click with empty input | [ ] |
| 201 | Loading state during translation | "Translating..." visible | [ ] |
| 202 | Job activation | Click history job → editor updates | [ ] |
| 203 | Model selector | Options present | [ ] |

### 2.11 AppShell Layout — NOT TESTED

| # | Interaction | Test | Status |
|---|-------------|------|--------|
| 204 | Renders sidebar + topbar + routerview | Sub-components present | [ ] |
| 205 | Cmd+K toggles command palette | paletteOpen changes | [ ] |
| 206 | Cmd+` toggles sidebar | sidebarOpen changes | [ ] |
| 207 | Navigate handler closes sidebar + pushes route | Sidebar closes | [ ] |

### 2.12 Router (`src/router.ts`) — NOT TESTED

| # | Interaction | Test | Status |
|---|-------------|------|--------|
| 208 | `/` redirects to `/review` | Route redirect | [ ] |
| 209 | Unknown path → `/review` | Catch-all redirect | [ ] |
| 210 | `/login` accessible when unauthenticated | Public path | [ ] |
| 211 | Authenticated user on `/login` → `/review` | Redirect away from login | [ ] |

### 2.13 Backend Client Mock Functions (`src/modules/backend/backendClient.ts`) — NOT TESTED

| # | Function | Test | Status |
|---|----------|------|--------|
| 212 | `mockRegisterReview` | Returns ReviewSummary[] with sessionIds | [ ] |
| 213 | `mockInitialReview` | Returns sessionId + messages + summary | [ ] |
| 214 | `mockDocumentChat` | Returns assistant ChatMessage with content | [ ] |
| 215 | `mockResearchRun` | Returns answer + citations | [ ] |
| 216 | `mockTranslationRun` | Returns jobId + translatedText | [ ] |
| 217 | `mockEvalsRun` | Returns benchmarkId + status | [ ] |
| 218 | `mockSaveSettings` | Resolves void | [ ] |
| 219 | `mockTestSync` | Returns ok + message | [ ] |
| 220 | `backendRegistry` | Contains expected command docs | [ ] |

---

## 3. Rust — Unit + Integration Tests

### 3.1 resolve_url (Unit — existing; two-branch rule since #41, never a cloud URL)

| # | Test | Status |
|---|------|--------|
| 221 | Premium refuses every path ("requires Elefant Premium") until the L0 payload contract | [x] |
| 222 | Ollama/BYOK refuse the cloud contract (`/api/v1`, `/api/v1/…`), judged on the parsed, case-folded path | [x] |
| 223 | Ollama routes sidecar paths to sidecar | [x] |
| 224 | BYOK routes same as Ollama | [x] |
| 225 | Unknown path → sidecar | [x] |
| 226 | No sidecar → error for sidecar path | [x] |
| 227 | A path that would change the host (`@evil.example/x`, `//host/x`) → error | [x] |
| 228 | Lookalike prefixes (`/api/v10/x`, `/api/v1x`) → sidecar; no result is ever non-loopback | [x] |

### 3.2 scan_folder (Unit — missing)

| # | Test | Status |
|---|------|--------|
| 229 | Scans directory with files → Vec<FileInfo> | [ ] |
| 230 | Empty directory → empty Vec | [ ] |
| 231 | Nested directories → all files found | [ ] |
| 232 | File metadata: path, size, modified populated | [ ] |
| 233 | Nonexistent path → error | [ ] |

### 3.3 move_to_trash (Unit — missing)

| # | Test | Status |
|---|------|--------|
| 234 | Valid file → Ok(()) | [ ] |
| 235 | Nonexistent file → Err | [ ] |

### 3.4 Backend Mode (Unit — missing)

| # | Test | Status |
|---|------|--------|
| 236 | `get_backend_mode` returns serialized mode | [ ] |
| 237 | `set_backend_mode("ollama")` → BackendMode::Ollama | [ ] |
| 238 | `set_backend_mode("byok")` → BackendMode::Byok | [ ] |
| 239 | `set_backend_mode("premium")` → BackendMode::Premium | [ ] |
| 240 | `set_backend_mode("invalid")` → Err | [ ] |

### 3.5 Serde Roundtrip (Unit — missing)

| # | Test | Status |
|---|------|--------|
| 241 | BackendMode serializes to lowercase | [ ] |
| 242 | BackendMode deserializes from lowercase | [ ] |
| 243 | FileInfo serializes to JSON | [ ] |

### 3.6 Per-Mode Routing (Unit — `CLOUD_ONLY_PREFIXES` retired by the two-branch rule, #41)

| # | Test | Status |
|---|------|--------|
| 244 | Ollama/BYOK route every non-`/api/v1` path to the sidecar | [x] |
| 245 | Ollama/BYOK refuse `/api/v1` and `/api/v1/…` with "requires Elefant Premium" | [x] |
| 246 | Lookalikes (`/api/v10/x`, `/api/v1x`) route to the sidecar; Premium refuses every path | [x] |

---

## Summary

| Layer | Total | Tested | Missing | Coverage |
|-------|-------|--------|---------|----------|
| Python Sidecar (HTTP endpoints) | 65 | 48 | 17 | 74% |
| Python Sidecar (DB service) | 24 | 16 | 8 | 67% |
| Python Sidecar (other services) | 8 | 6 | 2 | 75% |
| TypeScript Utilities | 13 | 13 | 0 | 100% |
| TypeScript API Client | 13 | 10 | 3 | 77% |
| TypeScript Components (tested) | 41 | 38 | 3 | 93% |
| TypeScript Components (untested) | 43 | 0 | 43 | 0% |
| TypeScript Backend Client | 9 | 0 | 9 | 0% |
| TypeScript Router | 4 | 0 | 4 | 0% |
| Rust (resolve_url) | 8 | 8 | 0 | 100% |
| Rust (other functions) | 18 | 0 | 18 | 0% |
| **TOTAL** | **246** | **139** | **107** | **57%** |
