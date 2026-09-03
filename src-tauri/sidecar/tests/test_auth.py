# ABOUTME: Tests for the shared-secret middleware in app.py.
# ABOUTME: Covers open access when no secret is configured, and enforcement when one is.
from httpx import ASGITransport, AsyncClient

from app import create_app
from main import resolve_secret


async def test_no_secret_configured_allows_requests():
    app = create_app(secret=None)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.get("/health")
    assert resp.status_code == 200


async def test_secret_configured_rejects_missing_header():
    app = create_app(secret="test-secret-123")
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.get("/health")
    assert resp.status_code == 401


async def test_secret_configured_rejects_wrong_secret():
    app = create_app(secret="test-secret-123")
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.get("/health", headers={"x-rumble-secret": "wrong-value"})
    assert resp.status_code == 401


async def test_secret_configured_accepts_correct_secret():
    app = create_app(secret="test-secret-123")
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.get("/health", headers={"x-rumble-secret": "test-secret-123"})
    assert resp.status_code == 200

def test_secret_prefers_env_over_flag(monkeypatch):
    monkeypatch.setenv("RUMBLE_SIDECAR_SECRET", "from-env")
    assert resolve_secret("from-flag") == "from-env"


def test_secret_falls_back_to_flag(monkeypatch):
    monkeypatch.delenv("RUMBLE_SIDECAR_SECRET", raising=False)
    assert resolve_secret("from-flag") == "from-flag"


def test_secret_is_none_when_neither_provided(monkeypatch):
    monkeypatch.delenv("RUMBLE_SIDECAR_SECRET", raising=False)
    assert resolve_secret(None) is None

