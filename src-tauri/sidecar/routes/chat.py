# ABOUTME: Chat CRUD and messaging endpoints for the sidecar.
# ABOUTME: Handles chat sessions, message history, sync responses, and SSE streaming.

import json
from typing import Any

from fastapi import APIRouter, HTTPException
from sse_starlette.sse import EventSourceResponse

from services import db, llm

router = APIRouter(prefix="/chats", tags=["chat"])


@router.post("")
async def create_chat(body: dict[str, Any] | None = None) -> dict:
    title = (body or {}).get("title")
    chat = await db.create_chat(title=title)
    return {"id": chat["id"]}


@router.get("")
async def list_chats() -> dict:
    chats = await db.list_chats()
    return {"chats": chats}


@router.get("/{chat_id}")
async def get_chat(chat_id: str) -> dict:
    chat = await db.get_chat(chat_id)
    if not chat:
        raise HTTPException(status_code=404, detail="chat not found")
    messages = await db.get_messages(chat_id)
    return {**chat, "messages": messages}


@router.delete("/{chat_id}")
async def delete_chat(chat_id: str) -> dict:
    deleted = await db.delete_chat(chat_id)
    if not deleted:
        raise HTTPException(status_code=404, detail="chat not found")
    return {"status": "ok"}


@router.patch("/{chat_id}")
async def update_chat(chat_id: str, body: dict[str, Any]) -> dict:
    chat = await db.update_chat(chat_id, title=body.get("title"))
    if not chat:
        raise HTTPException(status_code=404, detail="chat not found")
    return chat


@router.get("/{chat_id}/messages")
async def get_messages(chat_id: str) -> dict:
    chat = await db.get_chat(chat_id)
    if not chat:
        raise HTTPException(status_code=404, detail="chat not found")
    messages = await db.get_messages(chat_id)
    return {"chat_id": chat_id, "messages": messages}


@router.post("/{chat_id}/message")
async def send_message(chat_id: str, body: dict[str, Any]) -> dict:
    chat = await db.get_chat(chat_id)
    if not chat:
        raise HTTPException(status_code=404, detail="chat not found")

    text = body.get("text", "")
    if not text:
        raise HTTPException(status_code=422, detail="text is required")

    model_name = body.get("model") or None

    # Store user message
    await db.add_message(chat_id, "user", text)

    # Get conversation history for context
    messages = await db.get_messages(chat_id)

    # Get AI response
    try:
        response_text = await llm.send_message(messages, model_name=model_name)
    except Exception as exc:
        raise HTTPException(status_code=502, detail=f"LLM error: {exc}") from exc

    # Store assistant message
    assistant_msg = await db.add_message(chat_id, "assistant", response_text)
    return assistant_msg


@router.post("/{chat_id}/stream")
async def stream_message(chat_id: str, body: dict[str, Any]) -> EventSourceResponse:
    chat = await db.get_chat(chat_id)
    if not chat:
        raise HTTPException(status_code=404, detail="chat not found")

    text = body.get("text", "")
    if not text:
        raise HTTPException(status_code=422, detail="text is required")

    model_name = body.get("model") or None

    # Store user message
    await db.add_message(chat_id, "user", text)

    # Get conversation history
    messages = await db.get_messages(chat_id)

    async def event_generator():
        full_response = []
        try:
            yield {"event": "message", "data": json.dumps({"type": "status", "value": "generating"})}
            async for chunk in llm.stream_message(messages, model_name=model_name):
                full_response.append(chunk)
                yield {"event": "message", "data": json.dumps({"type": "delta", "value": chunk})}

            # Store complete assistant message
            complete_text = "".join(full_response)
            await db.add_message(chat_id, "assistant", complete_text)

            yield {"event": "message", "data": json.dumps({"type": "done", "value": complete_text})}
        except Exception as exc:
            yield {"event": "message", "data": json.dumps({"type": "error", "value": str(exc)})}

    return EventSourceResponse(event_generator())
