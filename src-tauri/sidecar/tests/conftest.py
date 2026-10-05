# ABOUTME: Shared fixtures for sidecar tests.
# ABOUTME: Provides ephemeral DB, FastAPI test client, LLM mock, and a real loopback fake Ollama server.

import json
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
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
        # Plain prose, not JSON (rumble#46): the draft pipeline treats this
        # raw text as the draft directly, it doesn't parse it as JSON.
        return f"Draft: {user_text}"
    if "review" in system_prompt.lower():
        return json.dumps({"summary": f"Review: {user_text}", "issues": []})
    if "research" in system_prompt.lower():
        return json.dumps({"result": f"Research: {user_text}", "sources": []})
    return json.dumps({"text": user_text})


# serve_forever's shutdown poll; its 0.5s default would add half a second to every test's teardown.
_SERVER_POLL_S = 0.01


class RecordingServer:
    """A real HTTP server on 127.0.0.1 that records each request and answers from `responses` (path -> JSON)."""

    def __init__(self, responses: dict[str, dict]):
        self.responses = responses
        self.requests: list[dict] = []
        server = self

        class _Handler(BaseHTTPRequestHandler):
            def _answer(self):
                length = int(self.headers.get("Content-Length") or 0)
                self.rfile.read(length)
                server.requests.append({"method": self.command, "path": self.path, "headers": dict(self.headers)})
                body = server.responses.get(self.path)
                payload = json.dumps(body if body is not None else {"error": "not found"}).encode()
                self.send_response(200 if body is not None else 404)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

            do_GET = do_POST = _answer

            def log_message(self, *_args):
                pass

        self._httpd = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
        self.url = f"http://127.0.0.1:{self._httpd.server_address[1]}"
        threading.Thread(target=self._httpd.serve_forever, args=(_SERVER_POLL_S,), daemon=True).start()

    def set_tags(self, *names: str, remote: tuple[str, ...] = ()):
        """Serve these /api/tags items; names in `remote` carry the remote_host/remote_model Ollama sets on cloud models."""
        self.responses["/api/tags"] = {
            "models": [
                {"name": n, **({"remote_host": "https://ollama.com:443", "remote_model": n} if n in remote else {})}
                for n in names
            ]
        }

    def close(self):
        self._httpd.shutdown()
        self._httpd.server_close()


CHAT_COMPLETION = {
    "id": "chatcmpl-test",
    "object": "chat.completion",
    "created": 0,
    "model": "llama3.2:latest",
    "choices": [{"index": 0, "message": {"role": "assistant", "content": "local reply"}, "finish_reason": "stop"}],
}


@pytest.fixture
def fake_ollama(monkeypatch):
    """A real loopback Ollama stand-in: `.set_tags(...)` what it serves, read `.requests` for what it received."""
    server = RecordingServer({"/api/tags": {"models": []}, "/v1/chat/completions": CHAT_COMPLETION})
    monkeypatch.setenv("OLLAMA_BASE_URL", server.url)
    yield server
    server.close()


_ROUTING_ENVS = ("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy", "OPENAI_BASE_URL")


@pytest.fixture
def hostile_env(monkeypatch):
    """Every inherited env that could re-route or re-key sidecar traffic; routing ones point at this recording decoy.

    httpx reads proxy envs when a client is built, so llm's cached Ollama client is dropped on both sides:
    one built by an earlier test, before these envs existed, would pass whatever its trust_env.
    """
    from services import llm

    llm._loopback_http_client.cache_clear()
    decoy = RecordingServer({})
    for name in _ROUTING_ENVS:
        monkeypatch.setenv(name, decoy.url)
    monkeypatch.delenv("NO_PROXY", raising=False)
    monkeypatch.delenv("no_proxy", raising=False)
    monkeypatch.setenv("OPENAI_API_KEY", "env-openai-key")
    monkeypatch.setenv("OPENAI_ORG_ID", "env-org")
    monkeypatch.setenv("OPENAI_PROJECT_ID", "env-project")
    monkeypatch.setenv("BYOK_API_KEY", "byok-key")
    yield decoy
    llm._loopback_http_client.cache_clear()
    decoy.close()


@pytest.fixture(autouse=True)
def mock_llm():
    """Patch LLM calls so tests never hit a real model provider."""
    with (
        patch("services.llm.send_message", new=AsyncMock(side_effect=_fake_send_message)),
        patch("services.llm.stream_message", new=_fake_stream_message),
        patch("services.llm.run_single_turn", new=AsyncMock(side_effect=_fake_run_single_turn)),
    ):
        yield
