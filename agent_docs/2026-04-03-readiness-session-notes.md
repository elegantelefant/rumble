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
