# ABOUTME: Unit tests for _build_agent's provider selection logic.
# ABOUTME: Covers mode-driven branching (never key-presence-driven) and cloud-model refusal.
from unittest.mock import MagicMock

import httpx
import pytest

from services import llm
from tests.conftest import CHAT_COMPLETION


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
    _never_resolve_ollama_model(monkeypatch)

    agent = await llm._build_agent(api_key="explicit-key")

    assert agent.model.client.api_key == "explicit-key"


async def test_build_agent_byok_mode_falls_back_to_env_key(monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "byok")
    monkeypatch.setenv("BYOK_API_KEY", "env-key-123")
    _never_resolve_ollama_model(monkeypatch)

    agent = await llm._build_agent()

    assert agent.model.client.api_key == "env-key-123"


async def test_build_agent_byok_mode_prefers_explicit_key_over_env(monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "byok")
    monkeypatch.setenv("BYOK_API_KEY", "env-key-123")
    _never_resolve_ollama_model(monkeypatch)

    agent = await llm._build_agent(api_key="explicit-key")

    assert agent.model.client.api_key == "explicit-key"


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

    async def fake_resolve(_model_name=None):
        return "llama3.2"

    monkeypatch.setattr(llm, "_resolve_ollama_model", fake_resolve)

    client = (await llm._build_agent()).model.client

    assert (str(client.base_url), client.api_key) == (f"{llm.mode.ollama_openai_base_url()}/", "ollama")


async def test_build_agent_missing_mode_env_defaults_to_ollama(monkeypatch):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.delenv("BYOK_API_KEY", raising=False)

    async def fake_resolve(_model_name=None):
        return "llama3.2"

    monkeypatch.setattr(llm, "_resolve_ollama_model", fake_resolve)

    client = (await llm._build_agent()).model.client

    assert (str(client.base_url), client.api_key) == (f"{llm.mode.ollama_openai_base_url()}/", "ollama")


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


@pytest.mark.parametrize(
    ("default", "requested", "refusal"),
    [
        ("llama3.2", None, "remote"),  # the configured default
        ("not-pulled", None, "no local Ollama model"),  # the first-pulled fallback
        ("not-pulled", "llama3.2", "remote"),  # a model requested by name
    ],
)
async def test_build_agent_refuses_model_ollama_marks_remote_after_an_earlier_local_resolution(
    monkeypatch, fake_ollama, default, requested, refusal
):
    # e.g. `ollama cp gpt-oss:120b-cloud llama3.2` while the sidecar runs
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.setenv("OLLAMA_DEFAULT_MODEL", default)
    fake_ollama.set_tags("llama3.2:latest")
    _mock_provider(monkeypatch)
    await llm._build_agent(model_name=requested)
    fake_ollama.set_tags("llama3.2:latest", remote=("llama3.2:latest",))

    with pytest.raises(ValueError, match=refusal):
        await llm._build_agent(model_name=requested)


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


def _lowercase_headers(request: dict) -> dict:
    return {k.lower(): v for k, v in request["headers"].items()}


async def test_ollama_mode_traffic_bypasses_inherited_proxy_env(monkeypatch, fake_ollama, hostile_env):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "ollama")
    fake_ollama.set_tags("llama3.2:latest")

    await (await llm._build_agent()).run("hi")

    assert (hostile_env.requests, [r["path"] for r in fake_ollama.requests]) == ([], ["/api/tags", "/v1/chat/completions"])


async def test_ollama_mode_completion_ignores_openai_env_and_byok_key(monkeypatch, fake_ollama, hostile_env):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "ollama")
    fake_ollama.set_tags("llama3.2:latest")

    await (await llm._build_agent()).run("hi")

    headers = _lowercase_headers(fake_ollama.requests[-1])
    assert (headers["authorization"], "openai-organization" in headers, "openai-project" in headers) == (
        "Bearer ollama",
        False,
        False,
    )


async def test_byok_mode_sends_to_public_openai_host_despite_openai_base_url(monkeypatch, hostile_env):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "byok")
    sent: list[httpx.Request] = []

    def _record(request: httpx.Request) -> httpx.Response:
        sent.append(request)
        return httpx.Response(200, json=CHAT_COMPLETION)

    monkeypatch.setattr(
        llm, "cached_async_http_client", lambda provider: httpx.AsyncClient(transport=httpx.MockTransport(_record))
    )

    await (await llm._build_agent()).run("hi")

    assert (str(sent[0].url), sent[0].headers["authorization"], "openai-organization" in sent[0].headers) == (
        f"{llm.OPENAI_API_BASE_URL}/chat/completions",
        "Bearer byok-key",
        False,
    )
