# Readiness Wiggum — Session Notes

> Observations, surprises, and non-obvious findings from readiness implementation.

---

## Session Notes

### Phase 1 verification + Phase 2A (_parse_llm_json fix) — 2026-04-02

**P1.1-P1.2, P1.4-P1.5, P1.7 confirmed by code inspection.** All code-verifiable Phase 1 items check out. Remaining P1 items (P1.3, P1.6, P1.8-P1.13) require runtime testing.

**Old `_parse_llm_json` only handled fences at start of string.** `text.startswith("```")` missed the common LLM pattern of "Here is the result:\n```json\n...". The split-on-backticks approach handles preamble, postamble, and missing language tags.

**`_parse_job_json` had duplicated parsing logic.** Refactored to delegate to `_parse_llm_json`, converting the HTTPException to ValueError (since jobs run in background and can't use HTTP exceptions directly).

**Existing test in `test_edge_cases.py` asserted old error message.** The test checked for "invalid JSON" in the detail — updated to match the new "unparseable" wording.

### Phase 2B-D verification — 2026-04-02

**Default model mismatch is a deployment blocker.** The hardcoded `OLLAMA_DEFAULT_MODEL=llama3.2` fails on any system that doesn't have that exact model pulled. Fixed with `_resolve_ollama_model()` that queries Ollama at first use and caches the result. This is a latent issue that would hit every new user.

**DeepSeek-R1 `<think>` tags are intermittent.** The model sometimes outputs `<think>...</think>` blocks before JSON, sometimes not. This made the chat_title endpoint flaky — first call fails, second succeeds. The regex strip in `_parse_llm_json` handles this consistently.

**Multi-turn chat works correctly.** The `_to_message_history()` design is sound — excludes the last message because PydanticAI's `agent.run(user_text, message_history=...)` sends user_text as the new prompt. Tested with "My name is Alice" / "What is my name?" — context retained.

**Research sources schema mismatch.** The LLM returns sources as strings or partial dicts, but `ResearchResultResponse.sources` expects `list[SearchResult]` with required `id` and `title` fields and `extra='forbid'`. Since we can't modify `generated.py`, added `_normalize_sources()` to bridge the gap.

**All AI endpoints verified.** Clarify, translate, summarise/document, summarise/chat, summarise/search, chat_title, draft, review, and research all return valid responses with the auto-resolved model.

### Phase 3A — shared API helper — 2026-04-02

**P3.1 complete.** Created `src/api/sidecar.ts` with typed wrappers for all sidecar endpoints. Reuses generated Orval types (ClarifyRequest, TranslateResponse, etc.) directly — no type duplication. Chat-related types (SidecarMessage, SidecarChat) defined locally since the chat endpoints use untyped dicts in Python.

**Polling helpers included.** `waitForJob()` and `waitForResearch()` poll at 1.5s intervals with a 3-minute timeout. These abstract the create-then-poll pattern that draft/review/research views all need.

**No lint config in this project.** Unlike the Nuxt monorepo, Rumble doesn't have ESLint or perfectionist configured. Verified with `vue-tsc --noEmit` (type-checks clean) and `pnpm vitest run` (129/129 pass).

### Phase 3B — Wire Document Review — 2026-04-02

**File reading uses FileReader.readAsText().** This works for `.txt` files. For `.pdf` and `.docx`, the browser reads raw bytes as text — the result will be garbled/unusable. A proper solution needs a file parsing library (e.g. `pdfjs-dist` for PDF, `mammoth` for DOCX). For now, only `.txt` files will produce meaningful reviews.

**Chat context seeding.** After the review job completes, we create a sidecar chat and seed it with the full document text + review summary as the first message. This gives the LLM full context for follow-up questions. Trade-off: the seeding `sendMessage` call generates an LLM response we discard. An alternative would be to directly insert messages into the DB, but that would require a new sidecar endpoint.

**Streaming (spec 3.4) not wired.** The view uses sync `sendMessage` rather than SSE `streamMessage`. SSE through Tauri IPC would require a different approach (Tauri event listener rather than invoke). Left as a separate task.

**Removed demo seed session.** The initial Contract_2024.pdf demo session was hardcoded mock data. Replaced with an empty sessions dict so the view starts clean.

**Spec items 2.1–2.4 checked off retroactively.** These were verified in the previous session (P2.4–P2.7 in imp plan) but never marked in the spec.
