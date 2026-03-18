# ABOUTME: Error propagation tests — verify behavior when LLM calls fail.
# ABOUTME: Overrides the autouse mock_llm per-test to inject failures.

import asyncio
import json
from unittest.mock import AsyncMock, patch

import pytest


# --- SSE parser (duplicated from test_chat.py — 8 lines, not worth abstracting) ---


def _parse_sse(body: str) -> list[dict]:
    """Extract JSON payloads from an SSE response body."""
    events = []
    for line in body.splitlines():
        if line.startswith("data:"):
            raw = line[len("data:"):].strip()
            if raw:
                try:
                    events.append(json.loads(raw))
                except json.JSONDecodeError:
                    pass
    return events


# --- Chat send_message failures ---


async def test_send_message_llm_failure_returns_502(client, chat_id):
    with patch("services.llm.send_message", new=AsyncMock(side_effect=RuntimeError("model exploded"))):
        resp = await client.post(f"/chats/{chat_id}/message", json={"text": "Hello"})
    assert resp.status_code == 502
    assert "LLM error" in resp.json()["detail"]


async def test_send_message_llm_failure_cleans_up_user_message(client, chat_id):
    with patch("services.llm.send_message", new=AsyncMock(side_effect=RuntimeError("model exploded"))):
        await client.post(f"/chats/{chat_id}/message", json={"text": "Hello"})
    resp = await client.get(f"/chats/{chat_id}/messages")
    messages = resp.json()["messages"]
    assert len(messages) == 0


# --- Chat stream failures ---


async def _failing_stream(messages, system_prompt=None, model_name=None, api_key=None):
    """Yield nothing, then raise."""
    raise RuntimeError("stream broke")
    yield  # noqa: RET503 — make this an async generator


async def _partial_then_fail_stream(messages, system_prompt=None, model_name=None, api_key=None):
    """Yield two chunks then raise."""
    yield "chunk1 "
    yield "chunk2 "
    raise RuntimeError("mid-stream failure")


async def test_stream_llm_failure_emits_error_event(client, chat_id):
    with patch("services.llm.stream_message", new=_failing_stream):
        resp = await client.post(f"/chats/{chat_id}/stream", json={"text": "Hello"})
    assert resp.status_code == 200
    events = _parse_sse(resp.text)
    types = [e["type"] for e in events]
    assert "error" in types


async def test_stream_llm_failure_cleans_up_user_message(client, chat_id):
    with patch("services.llm.stream_message", new=_failing_stream):
        await client.post(f"/chats/{chat_id}/stream", json={"text": "Hello"})
    resp = await client.get(f"/chats/{chat_id}/messages")
    messages = resp.json()["messages"]
    assert len(messages) == 0


async def test_stream_partial_events_before_error(client, chat_id):
    with patch("services.llm.stream_message", new=_partial_then_fail_stream):
        resp = await client.post(f"/chats/{chat_id}/stream", json={"text": "Hello"})
    events = _parse_sse(resp.text)
    types = [e["type"] for e in events]
    assert types[0] == "status"
    assert "delta" in types
    assert types[-1] == "error"
    assert "done" not in types


# --- Job failures ---


async def test_draft_job_llm_failure_stores_error(client):
    with patch("services.llm.run_single_turn", new=AsyncMock(side_effect=RuntimeError("draft failed"))):
        resp = await client.post("/draft", json={"prompt": "Draft an NDA"})
        job_id = resp.json()["job_id"]
        await asyncio.sleep(0.3)
    resp = await client.get(f"/draft/{job_id}/result")
    data = resp.json()
    assert data["status"] == "failed"
    assert data["result"] is None


async def test_review_job_llm_failure_stores_error(client):
    with patch("services.llm.run_single_turn", new=AsyncMock(side_effect=RuntimeError("review failed"))):
        resp = await client.post("/review", json={"text": "Review this clause..."})
        job_id = resp.json()["job_id"]
        await asyncio.sleep(0.3)
    resp = await client.get(f"/review/{job_id}/result")
    data = resp.json()
    assert data["status"] == "failed"
    assert data["result"] is None


async def test_research_job_llm_failure_stores_error(client):
    with patch("services.llm.run_single_turn", new=AsyncMock(side_effect=RuntimeError("research failed"))):
        resp = await client.post("/research", json={"question": "What is estoppel?"})
        report_id = resp.json()["report_id"]
        await asyncio.sleep(0.3)
    resp = await client.get(f"/research/{report_id}/result")
    data = resp.json()
    assert data["status"] == "failed"


# --- Stateless AI endpoint failures ---


async def test_clarify_llm_failure_propagates(client):
    """Stateless AI routes don't catch LLM exceptions — they propagate unhandled."""
    with (
        patch("services.llm.run_single_turn", new=AsyncMock(side_effect=RuntimeError("clarify boom"))),
        pytest.raises(RuntimeError, match="clarify boom"),
    ):
        await client.post("/clarify", json={"ask": "What is tort?"})


async def test_translate_llm_failure_propagates(client):
    """Stateless AI routes don't catch LLM exceptions — they propagate unhandled."""
    with (
        patch("services.llm.run_single_turn", new=AsyncMock(side_effect=RuntimeError("translate boom"))),
        pytest.raises(RuntimeError, match="translate boom"),
    ):
        await client.post("/translate", json={"text": "Hello", "target_lang": "es"})
