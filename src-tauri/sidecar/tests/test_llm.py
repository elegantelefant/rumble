# ABOUTME: Unit tests for _build_agent's provider selection logic.
# ABOUTME: Covers explicit api_key arg, BYOK_API_KEY env var fallback, and Ollama default.
from unittest.mock import MagicMock

from services import llm


def _mock_provider(monkeypatch):
    mock_cls = MagicMock(return_value=MagicMock())
    monkeypatch.setattr(llm, "OpenAIProvider", mock_cls)
    return mock_cls


async def test_build_agent_uses_explicit_api_key(monkeypatch):
    mock_cls = _mock_provider(monkeypatch)

    await llm._build_agent(api_key="explicit-key")

    mock_cls.assert_called_once_with(api_key="explicit-key")


async def test_build_agent_falls_back_to_byok_env_var(monkeypatch):
    monkeypatch.setenv("BYOK_API_KEY", "env-key-123")
    mock_cls = _mock_provider(monkeypatch)

    await llm._build_agent()

    mock_cls.assert_called_once_with(api_key="env-key-123")


async def test_build_agent_prefers_explicit_api_key_over_env_var(monkeypatch):
    monkeypatch.setenv("BYOK_API_KEY", "env-key-123")
    mock_cls = _mock_provider(monkeypatch)

    await llm._build_agent(api_key="explicit-key")

    mock_cls.assert_called_once_with(api_key="explicit-key")


async def test_build_agent_uses_ollama_when_no_key_or_env_var(monkeypatch):
    monkeypatch.delenv("BYOK_API_KEY", raising=False)
    mock_cls = _mock_provider(monkeypatch)

    async def fake_resolve():
        return "llama3.2"

    monkeypatch.setattr(llm, "_resolve_ollama_model", fake_resolve)

    await llm._build_agent()

    mock_cls.assert_called_once_with(base_url=llm.OLLAMA_BASE_URL, api_key="ollama")
