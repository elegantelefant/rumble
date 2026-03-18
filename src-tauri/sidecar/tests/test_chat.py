# ABOUTME: Tests for chat CRUD and messaging endpoints.
# ABOUTME: Covers create, list, get, delete, update, message send, and SSE stream.

import json
from unittest.mock import AsyncMock, patch


# --- Chat CRUD via HTTP ---


async def test_create_chat(client):
    resp = await client.post("/chats", json={"title": "My chat"})
    assert resp.status_code == 200
    assert "id" in resp.json()


async def test_create_chat_without_body(client):
    resp = await client.post("/chats")
    assert resp.status_code == 200
    assert "id" in resp.json()


async def test_list_chats_empty(client):
    resp = await client.get("/chats")
    assert resp.status_code == 200
    assert resp.json()["chats"] == []


async def test_list_chats_after_create(client):
    await client.post("/chats", json={"title": "One"})
    await client.post("/chats", json={"title": "Two"})
    resp = await client.get("/chats")
    chats = resp.json()["chats"]
    assert len(chats) == 2


async def test_get_chat_with_messages(client, chat_id):
    resp = await client.get(f"/chats/{chat_id}")
    assert resp.status_code == 200
    data = resp.json()
    assert data["id"] == chat_id
    assert "messages" in data
    assert isinstance(data["messages"], list)


async def test_get_nonexistent_chat(client):
    resp = await client.get("/chats/no-such-id")
    assert resp.status_code == 404


async def test_delete_chat(client, chat_id):
    resp = await client.delete(f"/chats/{chat_id}")
    assert resp.status_code == 200
    assert resp.json()["status"] == "ok"
    # Verify gone
    assert (await client.get(f"/chats/{chat_id}")).status_code == 404


async def test_delete_nonexistent_chat(client):
    resp = await client.delete("/chats/no-such-id")
    assert resp.status_code == 404


async def test_update_chat_title(client, chat_id):
    resp = await client.patch(f"/chats/{chat_id}", json={"title": "Renamed"})
    assert resp.status_code == 200
    assert resp.json()["title"] == "Renamed"


# --- Messages ---


async def test_get_messages_empty_chat(client, chat_id):
    resp = await client.get(f"/chats/{chat_id}/messages")
    assert resp.status_code == 200
    assert resp.json()["messages"] == []


async def test_get_messages_nonexistent_chat(client):
    resp = await client.get("/chats/no-such-id/messages")
    assert resp.status_code == 404


async def test_send_message_returns_assistant_reply(client, chat_id):
    resp = await client.post(
        f"/chats/{chat_id}/message",
        json={"text": "What is contract law?"},
    )
    assert resp.status_code == 200
    data = resp.json()
    assert data["role"] == "assistant"
    assert "Echo:" in data["content"]


async def test_send_message_stores_both_messages(client, chat_id):
    await client.post(f"/chats/{chat_id}/message", json={"text": "Hello"})
    resp = await client.get(f"/chats/{chat_id}/messages")
    messages = resp.json()["messages"]
    assert len(messages) == 2
    assert messages[0]["role"] == "user"
    assert messages[0]["content"] == "Hello"
    assert messages[1]["role"] == "assistant"


async def test_send_message_missing_text(client, chat_id):
    resp = await client.post(f"/chats/{chat_id}/message", json={"text": ""})
    assert resp.status_code == 422


async def test_send_message_nonexistent_chat(client):
    resp = await client.post("/chats/no-such-id/message", json={"text": "Hello"})
    assert resp.status_code == 404


async def test_conversation_accumulates(client, chat_id):
    await client.post(f"/chats/{chat_id}/message", json={"text": "First"})
    await client.post(f"/chats/{chat_id}/message", json={"text": "Second"})
    resp = await client.get(f"/chats/{chat_id}/messages")
    messages = resp.json()["messages"]
    assert len(messages) == 4  # 2 user + 2 assistant


# --- SSE Streaming ---


async def test_stream_message(client, chat_id):
    resp = await client.post(
        f"/chats/{chat_id}/stream",
        json={"text": "Hello stream"},
    )
    assert resp.status_code == 200
    assert "text/event-stream" in resp.headers.get("content-type", "")

    # Parse SSE events from the response body
    events = _parse_sse(resp.text)
    types = [e["type"] for e in events]
    assert "status" in types
    assert "delta" in types
    assert "done" in types


async def test_stream_message_persists_messages(client, chat_id):
    await client.post(f"/chats/{chat_id}/stream", json={"text": "Stored?"})
    resp = await client.get(f"/chats/{chat_id}/messages")
    messages = resp.json()["messages"]
    assert len(messages) == 2
    assert messages[0]["role"] == "user"
    assert messages[1]["role"] == "assistant"


async def test_stream_nonexistent_chat(client):
    resp = await client.post("/chats/no-such-id/stream", json={"text": "Hello"})
    assert resp.status_code == 404


async def test_stream_missing_text(client, chat_id):
    resp = await client.post(f"/chats/{chat_id}/stream", json={"text": ""})
    assert resp.status_code == 422


async def test_send_message_llm_failure_cleans_up_dangling(client, chat_id):
    """On LLM failure, the user message should be deleted — no dangling messages."""
    with patch("services.llm.send_message", new=AsyncMock(side_effect=RuntimeError("boom"))):
        resp = await client.post(f"/chats/{chat_id}/message", json={"text": "Hello"})
    assert resp.status_code == 502
    msgs = await client.get(f"/chats/{chat_id}/messages")
    assert len(msgs.json()["messages"]) == 0


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
