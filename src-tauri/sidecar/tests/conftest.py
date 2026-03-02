# ABOUTME: Shared fixtures for sidecar tests.
# ABOUTME: Provides ephemeral DB, FastAPI test client, and LLM mock.

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
    application = create_app(data_dir=tmp_data_dir)

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


def _fake_send_message(messages, model_name=None, api_key=None):
    """Return a canned assistant reply based on the last user message."""
    user_text = messages[-1]["content"] if messages else ""
    return f"Echo: {user_text}"


async def _fake_stream_message(messages, model_name=None, api_key=None):
    """Yield canned chunks for streaming tests."""
    user_text = messages[-1]["content"] if messages else ""
    for word in f"Echo: {user_text}".split():
        yield word + " "


@pytest.fixture(autouse=True)
def mock_llm():
    """Patch LLM calls so tests never hit a real model provider."""
    with (
        patch("services.llm.send_message", new=AsyncMock(side_effect=_fake_send_message)),
        patch("services.llm.stream_message", new=_fake_stream_message),
    ):
        yield
