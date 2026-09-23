# ABOUTME: Single source of truth for the sidecar's operating mode and its Ollama base URL.
# ABOUTME: Both routes/health.py and services/llm.py read mode and config through here, never independently.

import logging
import os
from typing import Literal
from urllib.parse import urlparse

logger = logging.getLogger(__name__)

Mode = Literal["ollama", "byok"]

_LOOPBACK_HOSTS = {"127.0.0.1", "localhost", "::1"}

CLOUD_MODEL_SUFFIXES = (":cloud", "-cloud")


def current_mode() -> Mode:
    """Return the sidecar's operating mode, failing closed to "ollama" if the
    host's RUMBLE_BACKEND_MODE is missing or unrecognised — a spawn that
    forgot to set it, or set it to something the sidecar doesn't know (e.g.
    "premium", which never reaches the sidecar in practice), must never be
    read as byok.
    """
    raw = os.environ.get("RUMBLE_BACKEND_MODE")
    if raw == "byok":
        return "byok"
    if raw != "ollama":
        logger.warning("Unrecognised or missing RUMBLE_BACKEND_MODE=%r; failing closed to ollama", raw)
    return "ollama"


def ollama_api_base() -> str:
    """Return OLLAMA_BASE_URL (no path suffix), refusing anything non-loopback.

    Validated here, once, so every caller — the OpenAI-compatible provider
    in services/llm.py and the /api/tags calls in routes/health.py — gets
    the same guarantee instead of each reading the env var independently.
    """
    raw = os.environ.get("OLLAMA_BASE_URL", "http://localhost:11434")
    host = urlparse(raw).hostname
    if host not in _LOOPBACK_HOSTS:
        raise ValueError(f"OLLAMA_BASE_URL must be loopback, got {raw!r}")
    return raw


def ollama_openai_base_url() -> str:
    """Return the Ollama base URL with the /v1 suffix PydanticAI's OpenAI-compatible provider expects."""
    return f"{ollama_api_base()}/v1"


def is_cloud_model(name: str) -> bool:
    """Whether a model name is an Ollama cloud model, by Ollama's own naming convention.

    Known gap: on Windows, Ollama strips the -cloud suffix during `run` and
    API calls (ollama/ollama#16314), so a model could reach us with the
    suffix already removed by Ollama itself before this check ever sees it.
    This only catches the suffix when Ollama hasn't stripped it first.
    """
    return name.endswith(CLOUD_MODEL_SUFFIXES)
