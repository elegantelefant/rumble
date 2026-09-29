# ABOUTME: Single source of truth for the sidecar's operating mode, its Ollama base URL, and which models are local.
# ABOUTME: Both routes/health.py and services/llm.py read mode and config through here, never independently.

import functools
import logging
import os
from typing import Literal
from urllib.parse import urlparse

import httpx

logger = logging.getLogger(__name__)

Mode = Literal["ollama", "byok"]

_LOOPBACK_HOSTS = {"127.0.0.1", "localhost", "::1"}

CLOUD_MODEL_SUFFIXES = (":cloud", "-cloud")

DEFAULT_OLLAMA_MODEL = "llama3.2"

OLLAMA_TAGS_TIMEOUT_S = 5.0

# Ollama's implicit tag for an untagged name: "llama3.2" means "llama3.2:latest".
_IMPLICIT_TAG = ":latest"


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
        _warn_fail_closed(raw)
    return "ollama"


@functools.cache
def _warn_fail_closed(raw: str | None) -> None:
    """Warn once per distinct value: packaged builds pass no mode, and current_mode() runs on every request."""
    logger.warning("Unrecognised or missing RUMBLE_BACKEND_MODE=%r; failing closed to ollama", raw)


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

    The second line of defence: is_cloud_tag's remote metadata is the primary
    signal. Normalised first because Ollama itself matches names ignoring case
    and surrounding whitespace, so "x:120b-CLOUD " still resolves to the cloud model.

    Known gap: on Windows, Ollama strips the -cloud suffix during `run` and
    API calls (ollama/ollama#16314), so a model could reach us with the
    suffix already removed by Ollama itself before this check ever sees it.
    This only catches the suffix when Ollama hasn't stripped it first.
    """
    return name.strip().casefold().endswith(CLOUD_MODEL_SUFFIXES)


def is_cloud_tag(tag: dict) -> bool:
    """Whether an /api/tags item is a cloud model: Ollama sets remote_host/remote_model on those, whatever the name."""
    return bool(tag.get("remote_host") or tag.get("remote_model")) or is_cloud_model(tag["name"])


def local_model_names(tags: list[dict]) -> list[str]:
    return [tag["name"] for tag in tags if not is_cloud_tag(tag)]


def _canonical(name: str) -> str:
    name = name.strip().casefold()
    return name if ":" in name.rsplit("/", 1)[-1] else name + _IMPLICIT_TAG


def local_model(name: str, tags: list[dict]) -> str | None:
    """Return the /api/tags name `name` refers to, or None if it isn't pulled.

    Raises ValueError if it is a cloud model, by name or by Ollama's metadata.
    """
    if is_cloud_model(name):
        raise ValueError(f"refusing cloud model {name!r} in ollama mode")
    wanted = _canonical(name)
    for tag in tags:
        if _canonical(tag["name"]) == wanted:
            if is_cloud_tag(tag):
                raise ValueError(f"refusing cloud model {name!r} in ollama mode: Ollama reports it remote")
            return tag["name"]
    return None


def default_model(tags: list[dict]) -> str | None:
    """OLLAMA_DEFAULT_MODEL as local_model resolves it; a cloud default is refused, never skipped."""
    name = os.environ.get("OLLAMA_DEFAULT_MODEL", DEFAULT_OLLAMA_MODEL)
    try:
        return local_model(name, tags)
    except ValueError as exc:
        raise ValueError(f"OLLAMA_DEFAULT_MODEL: {exc}") from exc


async def ollama_tags() -> list[dict]:
    """Fetch Ollama's /api/tags items from the validated loopback base URL, never through a proxy (#73)."""
    async with httpx.AsyncClient(timeout=OLLAMA_TAGS_TIMEOUT_S, trust_env=False) as client:
        resp = await client.get(f"{ollama_api_base()}/api/tags")
        resp.raise_for_status()
        return resp.json().get("models", [])
