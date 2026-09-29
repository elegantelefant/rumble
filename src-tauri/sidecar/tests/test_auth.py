# ABOUTME: Tests for the shared-secret middleware in app.py and the fail-closed startup check in main.py.
# ABOUTME: Covers dev-mode open access, enforcement when a secret is set, and refusal to start without either.
from unittest.mock import patch

import pytest
from httpx import ASGITransport, AsyncClient

from app import create_app
from main import cli, resolve_secret


async def test_dev_mode_allows_requests_without_header():
    app = create_app(secret=None, dev=True)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.get("/health")
    assert resp.status_code == 200


def test_create_app_requires_secret_or_dev():
    with pytest.raises(ValueError):
        create_app(secret=None, dev=False)


def test_cli_exits_without_secret_or_dev(monkeypatch):
    # uvicorn is mocked so that a regression which lets cli() past the guard
    # fails on the assert_not_called() below instead of actually binding a
    # socket and hanging the test.
    monkeypatch.delenv("RUMBLE_SIDECAR_SECRET", raising=False)
    with patch("main.create_app") as mock_create_app, patch("main.uvicorn"):
        try:
            cli(["--port", "11435"])
        except SystemExit as exc:
            assert exc.code != 0
        mock_create_app.assert_not_called()


def test_cli_exits_with_empty_secret_and_no_dev(monkeypatch):
    monkeypatch.setenv("RUMBLE_SIDECAR_SECRET", "")
    with patch("main.create_app") as mock_create_app, patch("main.uvicorn"):
        try:
            cli(["--port", "11435"])
        except SystemExit as exc:
            assert exc.code != 0
        mock_create_app.assert_not_called()


def test_cli_exits_when_resolve_secret_returns_empty_string(monkeypatch):
    # Bypasses resolve_secret's own "" -> None collapsing by mocking it
    # outright, so this pins cli()'s own falsy check on `secret` rather than
    # relying on resolve_secret to have already turned "" into None.
    with (
        patch("main.resolve_secret", return_value=""),
        patch("main.create_app") as mock_create_app,
        patch("main.uvicorn"),
    ):
        try:
            cli(["--port", "11435"])
        except SystemExit as exc:
            assert exc.code != 0
        mock_create_app.assert_not_called()


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

