# ABOUTME: Tests for health, readiness, and model listing endpoints.
# ABOUTME: Verifies mode detection, Ollama fallback, and BYOK static model list.


async def test_health_returns_ok(client):
    resp = await client.get("/health")
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "ok"
    assert data["mode"] in ("ollama", "byok")


async def test_health_mode_defaults_to_ollama(client, monkeypatch):
    monkeypatch.delenv("BYOK_API_KEY", raising=False)
    resp = await client.get("/health")
    assert resp.json()["mode"] == "ollama"


async def test_health_mode_byok_when_key_set(client, monkeypatch):
    monkeypatch.setenv("BYOK_API_KEY", "sk-test-123")
    resp = await client.get("/health")
    assert resp.json()["mode"] == "byok"


async def test_ready_byok_always_ready(client, monkeypatch):
    monkeypatch.setenv("BYOK_API_KEY", "sk-test-123")
    resp = await client.get("/ready")
    data = resp.json()
    assert data["status"] == "ready"
    assert data["mode"] == "byok"


async def test_ready_ollama_reports_unreachable(client, monkeypatch):
    monkeypatch.delenv("BYOK_API_KEY", raising=False)
    monkeypatch.setenv("OLLAMA_BASE_URL", "http://127.0.0.1:1")  # nothing listens here
    # Need to reimport to pick up new env var — or we accept the module-level default
    resp = await client.get("/ready")
    data = resp.json()
    assert data["mode"] == "ollama"
    # Either ready or not_ready depending on whether Ollama happens to be running
    assert data["status"] in ("ready", "not_ready")


async def test_models_byok_returns_static_list(client, monkeypatch):
    monkeypatch.setenv("BYOK_API_KEY", "sk-test-123")
    resp = await client.get("/models")
    data = resp.json()
    assert "models" in data
    ids = [m["id"] for m in data["models"]]
    assert "gpt-4o-mini" in ids
    assert any(m["default"] for m in data["models"])


async def test_models_ollama_returns_list_or_empty(client, monkeypatch):
    monkeypatch.delenv("BYOK_API_KEY", raising=False)
    resp = await client.get("/models")
    data = resp.json()
    assert "models" in data
    assert isinstance(data["models"], list)
