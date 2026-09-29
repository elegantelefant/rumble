# ABOUTME: Shared fixtures for sidecar tests.
# ABOUTME: Provides ephemeral DB, FastAPI test client, and LLM mock.

import json
import sys
from pathlib import Path
from unittest.mock import AsyncMock, patch

import pytest
import pytest_asyncio
from httpx import ASGITransport, AsyncClient

# Ensure the sidecar package root is importable
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from app import create_app
from services import db


@pytest_asyncio.fixture
async def tmp_data_dir(tmp_path):
    """Provide a temporary directory for the SQLite database."""
    return str(tmp_path)


@pytest_asyncio.fixture
async def app(tmp_data_dir):
    """Create a fresh FastAPI app with an ephemeral database."""
    application = create_app(data_dir=tmp_data_dir, dev=True)

    # Manually trigger lifespan since httpx doesn't do it
    await db.init_db(tmp_data_dir)
    yield application
    await db.close_db()


@pytest_asyncio.fixture
async def client(app):
    """Async HTTP client wired to the FastAPI app (no real server needed)."""
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as c:
        yield c


@pytest_asyncio.fixture
async def chat_id(client):
    """Create a chat and return its ID — convenience for message tests."""
    resp = await client.post("/chats", json={"title": "Test chat"})
    return resp.json()["id"]


def _fake_send_message(messages, system_prompt=None, model_name=None, api_key=None):
    """Return a canned assistant reply based on the last user message."""
    user_text = messages[-1]["content"] if messages else ""
    return f"Echo: {user_text}"


async def _fake_stream_message(messages, system_prompt=None, model_name=None, api_key=None):
    """Yield canned chunks for streaming tests."""
    user_text = messages[-1]["content"] if messages else ""
    for word in f"Echo: {user_text}".split():
        yield word + " "


def _fake_run_single_turn(user_text, system_prompt, model_name=None, api_key=None):
    """Return canned JSON for stateless AI endpoints, keyed on system_prompt keywords."""
    if "clarif" in system_prompt.lower():
        return json.dumps({"clarified_ask": f"Clarified: {user_text}", "questions": ["What jurisdiction?"]})
    if "title" in system_prompt.lower():
        return json.dumps({"title": "Generated Title"})
    if "translat" in system_prompt.lower():
        return json.dumps({"translated_text": f"Translated: {user_text}"})
    if "search" in system_prompt.lower() and "summar" in system_prompt.lower():
        return json.dumps({"summary": f"Search summary: {user_text}", "key_points": ["point1"], "citations": ["cite1"]})
    if "chat" in system_prompt.lower() and "summar" in system_prompt.lower():
        return json.dumps({"summary": f"Chat summary: {user_text}", "key_points": ["point1"]})
    if "summar" in system_prompt.lower():
        return json.dumps({"summary": f"Summary: {user_text}", "key_points": ["point1"]})
    if "draft" in system_prompt.lower():
        return json.dumps({"draft": f"Draft: {user_text}", "warnings": []})
    if "review" in system_prompt.lower():
        return json.dumps({"summary": f"Review: {user_text}", "issues": []})
    if "research" in system_prompt.lower():
        return json.dumps({"result": f"Research: {user_text}", "sources": []})
    return json.dumps({"text": user_text})


@pytest.fixture(autouse=True)
def mock_llm():
    """Patch LLM calls so tests never hit a real model provider."""
    with (
        patch("services.llm.send_message", new=AsyncMock(side_effect=_fake_send_message)),
        patch("services.llm.stream_message", new=_fake_stream_message),
        patch("services.llm.run_single_turn", new=AsyncMock(side_effect=_fake_run_single_turn)),
    ):
        yield
