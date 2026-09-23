# ABOUTME: Unit tests for _build_agent's provider selection logic.
# ABOUTME: Covers mode-driven branching (never key-presence-driven) and cloud-model refusal.
from unittest.mock import MagicMock

import pytest

from services import llm


def _mock_provider(monkeypatch):
    mock_cls = MagicMock(return_value=MagicMock())
    monkeypatch.setattr(llm, "OpenAIProvider", mock_cls)
    return mock_cls


def _never_resolve_ollama_model(monkeypatch):
    """Fail the test if the byok path ever tries to resolve an Ollama model."""

    async def _fail():
        raise AssertionError("byok mode must never call _resolve_ollama_model")

    monkeypatch.setattr(llm, "_resolve_ollama_model", _fail)


async def test_build_agent_byok_mode_uses_key_no_ollama_call(monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "byok")
    mock_cls = _mock_provider(monkeypatch)
    _never_resolve_ollama_model(monkeypatch)

    await llm._build_agent(api_key="explicit-key")

    mock_cls.assert_called_once_with(api_key="explicit-key")


async def test_build_agent_byok_mode_falls_back_to_env_key(monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "byok")
    monkeypatch.setenv("BYOK_API_KEY", "env-key-123")
    mock_cls = _mock_provider(monkeypatch)
    _never_resolve_ollama_model(monkeypatch)

    await llm._build_agent()

    mock_cls.assert_called_once_with(api_key="env-key-123")


async def test_build_agent_byok_mode_prefers_explicit_key_over_env(monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "byok")
    monkeypatch.setenv("BYOK_API_KEY", "env-key-123")
    mock_cls = _mock_provider(monkeypatch)
    _never_resolve_ollama_model(monkeypatch)

    await llm._build_agent(api_key="explicit-key")

    mock_cls.assert_called_once_with(api_key="explicit-key")


async def test_build_agent_byok_mode_without_key_fails_no_fallback(monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "byok")
    monkeypatch.delenv("BYOK_API_KEY", raising=False)
    _mock_provider(monkeypatch)
    _never_resolve_ollama_model(monkeypatch)

    with pytest.raises(ValueError):
        await llm._build_agent()


async def test_build_agent_ollama_mode_ignores_byok_env_key(monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "ollama")
    monkeypatch.setenv("BYOK_API_KEY", "should-be-ignored")
    mock_cls = _mock_provider(monkeypatch)

    async def fake_resolve():
        return "llama3.2"

    monkeypatch.setattr(llm, "_resolve_ollama_model", fake_resolve)

    await llm._build_agent()

    mock_cls.assert_called_once_with(base_url=llm.mode.ollama_openai_base_url(), api_key="ollama")


async def test_build_agent_missing_mode_env_defaults_to_ollama(monkeypatch):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.delenv("BYOK_API_KEY", raising=False)
    mock_cls = _mock_provider(monkeypatch)

    async def fake_resolve():
        return "llama3.2"

    monkeypatch.setattr(llm, "_resolve_ollama_model", fake_resolve)

    await llm._build_agent()

    mock_cls.assert_called_once_with(base_url=llm.mode.ollama_openai_base_url(), api_key="ollama")


async def test_build_agent_refuses_non_loopback_ollama_base_url(monkeypatch):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.setenv("OLLAMA_BASE_URL", "http://evil.example.com:11434")
    _mock_provider(monkeypatch)

    with pytest.raises(ValueError):
        await llm._build_agent()


async def test_build_agent_refuses_cloud_model_by_name(monkeypatch):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    _mock_provider(monkeypatch)
    _never_resolve_ollama_model(monkeypatch)

    with pytest.raises(ValueError):
        await llm._build_agent(model_name="qwen3.5:cloud")


class _FakeTagsResponse:
    def __init__(self, models: list[str]):
        self._models = models

    def raise_for_status(self):
        pass

    def json(self):
        return {"models": [{"name": name} for name in self._models]}


class _FakeAsyncClient:
    def __init__(self, models: list[str]):
        self._models = models

    async def __aenter__(self):
        return self

    async def __aexit__(self, *exc):
        return False

    async def get(self, _url):
        return _FakeTagsResponse(self._models)


async def test_resolve_ollama_model_filters_cloud_models_from_candidates(monkeypatch):
    monkeypatch.setattr(llm, "_resolved_ollama_model", None)
    monkeypatch.setattr(llm, "OLLAMA_DEFAULT_MODEL", "llama3.2")
    monkeypatch.setattr(llm.httpx, "AsyncClient", lambda timeout=5.0: _FakeAsyncClient(["qwen3.5:cloud", "custom-model"]))

    resolved = await llm._resolve_ollama_model()

    assert resolved == "custom-model"
