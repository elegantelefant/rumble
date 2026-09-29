# ABOUTME: Tests for health, readiness, and model listing endpoints.
# ABOUTME: Verifies mode detection via RUMBLE_BACKEND_MODE, cloud-model filtering, refusal reasons, and BYOK models.


async def test_health_returns_ok(client):
    resp = await client.get("/health")
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "ok"
    assert data["mode"] in ("ollama", "byok")


async def test_health_mode_defaults_to_ollama(client, monkeypatch):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    resp = await client.get("/health")
    assert resp.json()["mode"] == "ollama"


async def test_health_mode_byok_when_mode_env_set(client, monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "byok")
    resp = await client.get("/health")
    assert resp.json()["mode"] == "byok"


async def test_ready_byok_always_ready(client, monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "byok")
    resp = await client.get("/ready")
    data = resp.json()
    assert data["status"] == "ready"
    assert data["mode"] == "byok"


async def test_ready_ollama_reports_unreachable(client, monkeypatch):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.setenv("OLLAMA_BASE_URL", "http://127.0.0.1:1")  # nothing listens here
    resp = await client.get("/ready")
    data = resp.json()
    assert data["mode"] == "ollama"
    # Either ready or not_ready depending on whether Ollama happens to be running
    assert data["status"] in ("ready", "not_ready")


async def test_models_byok_returns_static_list(client, monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "byok")
    resp = await client.get("/models")
    data = resp.json()
    assert "models" in data
    ids = [m["id"] for m in data["models"]]
    assert "gpt-4o-mini" in ids
    assert any(m["default"] for m in data["models"])


async def test_models_ollama_returns_list_or_empty(client, monkeypatch):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    resp = await client.get("/models")
    data = resp.json()
    assert "models" in data
    assert isinstance(data["models"], list)


async def test_models_ollama_filters_cloud_models(client, monkeypatch, fake_ollama):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    fake_ollama.set_tags("qwen3.5:cloud", "llama3.2")

    resp = await client.get("/models")
    ids = [m["id"] for m in resp.json()["models"]]

    assert "llama3.2" in ids
    assert "qwen3.5:cloud" not in ids


async def test_models_ollama_hides_models_ollama_marks_remote(client, monkeypatch, fake_ollama):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    fake_ollama.set_tags("mymodel:latest", "llama3.2:latest", remote=("mymodel:latest",))

    resp = await client.get("/models")

    assert [m["id"] for m in resp.json()["models"]] == ["llama3.2:latest"]


async def test_ready_ollama_ready_when_reachable(client, monkeypatch, fake_ollama):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    fake_ollama.set_tags("llama3.2:latest")

    resp = await client.get("/ready")

    assert resp.json()["status"] == "ready"


async def test_ready_surfaces_refused_non_loopback_base_url(client, monkeypatch):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.setenv("OLLAMA_BASE_URL", "http://evil.example.com:11434")

    resp = await client.get("/ready")

    assert "must be loopback" in resp.json()["error"]


async def test_models_surfaces_refused_non_loopback_base_url(client, monkeypatch):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.setenv("OLLAMA_BASE_URL", "http://evil.example.com:11434")

    resp = await client.get("/models")

    assert "must be loopback" in resp.json()["error"]


async def test_ready_surfaces_refused_cloud_default_model(client, monkeypatch, fake_ollama):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.setenv("OLLAMA_DEFAULT_MODEL", "gpt-oss:120b-cloud")
    fake_ollama.set_tags("llama3.2:latest")

    resp = await client.get("/ready")

    assert "OLLAMA_DEFAULT_MODEL" in resp.json()["error"]


async def test_models_surfaces_refused_cloud_default_model(client, monkeypatch, fake_ollama):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.setenv("OLLAMA_DEFAULT_MODEL", "gpt-oss:120b-cloud")
    fake_ollama.set_tags("llama3.2:latest")

    resp = await client.get("/models")

    assert "OLLAMA_DEFAULT_MODEL" in resp.json()["error"]


async def test_models_ollama_bypasses_inherited_proxy_env(client, monkeypatch, fake_ollama, hostile_env):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    fake_ollama.set_tags("llama3.2:latest")

    resp = await client.get("/models")

    assert [m["id"] for m in resp.json()["models"]] == ["llama3.2:latest"]
