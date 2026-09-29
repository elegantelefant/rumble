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

    async def fake_resolve(_model_name=None):
        return "llama3.2"

    monkeypatch.setattr(llm, "_resolve_ollama_model", fake_resolve)

    await llm._build_agent()

    mock_cls.assert_called_once_with(base_url=llm.mode.ollama_openai_base_url(), api_key="ollama")


async def test_build_agent_missing_mode_env_defaults_to_ollama(monkeypatch):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.delenv("BYOK_API_KEY", raising=False)
    mock_cls = _mock_provider(monkeypatch)

    async def fake_resolve(_model_name=None):
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


async def test_resolve_ollama_model_filters_cloud_models_from_candidates(monkeypatch, fake_ollama):
    monkeypatch.setenv("OLLAMA_DEFAULT_MODEL", "llama3.2")
    fake_ollama.set_tags("qwen3.5:cloud", "custom-model")

    resolved = await llm._resolve_ollama_model()

    assert resolved == "custom-model"


async def test_resolve_ollama_model_skips_models_ollama_marks_remote(monkeypatch, fake_ollama):
    monkeypatch.setenv("OLLAMA_DEFAULT_MODEL", "not-pulled")
    fake_ollama.set_tags("mymodel:latest", "llama3.2:latest", remote=("mymodel:latest",))

    assert await llm._resolve_ollama_model() == "llama3.2:latest"


@pytest.mark.parametrize(
    "pulled",
    [
        (),  # not pulled: the old fallback passed the default through unchecked
        ("gpt-oss:120b-cloud", "llama3.2:latest"),
    ],
)
async def test_build_agent_refuses_cloud_default_model_when_none_requested(monkeypatch, fake_ollama, pulled):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.setenv("OLLAMA_DEFAULT_MODEL", "gpt-oss:120b-cloud")
    fake_ollama.set_tags(*pulled, remote=("gpt-oss:120b-cloud",))
    _mock_provider(monkeypatch)

    with pytest.raises(ValueError, match="OLLAMA_DEFAULT_MODEL"):
        await llm._build_agent()


async def test_build_agent_refuses_default_model_ollama_marks_remote(monkeypatch, fake_ollama):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.setenv("OLLAMA_DEFAULT_MODEL", "mymodel")  # e.g. `ollama cp qwen3.5:cloud mymodel`
    fake_ollama.set_tags("mymodel:latest", "llama3.2:latest", remote=("mymodel:latest",))
    _mock_provider(monkeypatch)

    with pytest.raises(ValueError, match="remote"):
        await llm._build_agent()


async def test_build_agent_refuses_requested_model_ollama_marks_remote(monkeypatch, fake_ollama):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    fake_ollama.set_tags("mymodel:latest", remote=("mymodel:latest",))
    _mock_provider(monkeypatch)

    with pytest.raises(ValueError, match="remote"):
        await llm._build_agent(model_name="mymodel")


@pytest.mark.parametrize("name", ["gpt-oss:120b-CLOUD", "qwen3.5:cloud ", " gemma3-Cloud"])
async def test_build_agent_refuses_cloud_suffix_regardless_of_case_or_whitespace(monkeypatch, fake_ollama, name):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    _mock_provider(monkeypatch)

    with pytest.raises(ValueError, match="refusing cloud model"):
        await llm._build_agent(model_name=name)


async def test_build_agent_refuses_requested_model_that_is_not_pulled(monkeypatch, fake_ollama):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    fake_ollama.set_tags("llama3.2:latest")
    _mock_provider(monkeypatch)

    with pytest.raises(ValueError, match="not a pulled local"):
        await llm._build_agent(model_name="phi4")


async def test_build_agent_resolves_requested_name_to_the_pulled_tag(monkeypatch, fake_ollama):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    fake_ollama.set_tags("llama3.2:latest")
    _mock_provider(monkeypatch)

    agent = await llm._build_agent(model_name="Llama3.2 ")

    assert agent.model.model_name == "llama3.2:latest"
