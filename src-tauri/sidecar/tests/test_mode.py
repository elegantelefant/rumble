# ABOUTME: Unit tests for services/mode.py — the single source of truth for operating mode.
# ABOUTME: Covers fail-closed defaults, loopback enforcement, and cloud-model name detection.
import pytest

from services import mode


def test_current_mode_byok_when_set(monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "byok")
    assert mode.current_mode() == "byok"


def test_current_mode_ollama_when_set(monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "ollama")
    assert mode.current_mode() == "ollama"


def test_current_mode_missing_defaults_to_ollama(monkeypatch):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    assert mode.current_mode() == "ollama"


def test_current_mode_unrecognised_defaults_to_ollama(monkeypatch):
    monkeypatch.setenv("RUMBLE_BACKEND_MODE", "premium")
    assert mode.current_mode() == "ollama"


def test_ollama_api_base_accepts_loopback(monkeypatch):
    monkeypatch.setenv("OLLAMA_BASE_URL", "http://127.0.0.1:11434")
    assert mode.ollama_api_base() == "http://127.0.0.1:11434"


def test_ollama_api_base_rejects_non_loopback(monkeypatch):
    monkeypatch.setenv("OLLAMA_BASE_URL", "http://example.com:11434")
    with pytest.raises(ValueError):
        mode.ollama_api_base()


def test_ollama_openai_base_url_adds_v1_suffix(monkeypatch):
    monkeypatch.setenv("OLLAMA_BASE_URL", "http://localhost:11434")
    assert mode.ollama_openai_base_url() == "http://localhost:11434/v1"


@pytest.mark.parametrize(
    "url",
    [
        "http://127.0.0.1:11434",
        "http://localhost:11434",
        "http://[::1]:11434",
    ],
)
def test_ollama_api_base_accepts_loopback_forms(monkeypatch, url):
    monkeypatch.setenv("OLLAMA_BASE_URL", url)
    assert mode.ollama_api_base() == url


@pytest.mark.parametrize(
    "url",
    [
        "http://localhost.evil.com:11434",  # a naive prefix/substring check would wrongly accept this
        "http://127.0.0.1.evil.com:11434",  # same, via the IP form
        "http://0.0.0.0:11434",  # binds all interfaces, not loopback
        "http://192.168.1.5:11434",  # LAN, not local
    ],
)
def test_ollama_api_base_refuses_lookalike_hosts(monkeypatch, url):
    monkeypatch.setenv("OLLAMA_BASE_URL", url)
    with pytest.raises(ValueError):
        mode.ollama_api_base()


@pytest.mark.parametrize("name", ["qwen3.5:cloud", "gemma3-cloud", "gpt-oss:120b-cloud", "deepseek-v3.1:671b-cloud"])
def test_is_cloud_model_true(name):
    assert mode.is_cloud_model(name)


@pytest.mark.parametrize("name", ["llama3.2", "qwen3.5:latest", "llama3.2:latest"])
def test_is_cloud_model_false(name):
    assert not mode.is_cloud_model(name)
