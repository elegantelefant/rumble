# ABOUTME: Health and readiness endpoints for the sidecar.
# ABOUTME: Reports status, checks Ollama reachability, and lists available models.

import httpx
from fastapi import APIRouter

from services import mode as mode_service

router = APIRouter(tags=["health"])


def _current_mode() -> str:
    return mode_service.current_mode()


@router.get("/health")
async def health() -> dict:
    return {"status": "ok", "mode": _current_mode()}


@router.get("/ready")
async def ready() -> dict:
    current = _current_mode()
    if current == "byok":
        # BYOK mode: just check key is present
        return {"status": "ready", "mode": "byok"}

    # Ollama mode: check Ollama is reachable and the configured default isn't refused
    try:
        mode_service.default_model(await mode_service.ollama_tags())
    except (httpx.HTTPError, httpx.ConnectError, httpx.TimeoutException):
        return {"status": "not_ready", "mode": "ollama", "error": "ollama unreachable"}
    except ValueError as exc:
        return {"status": "not_ready", "mode": "ollama", "error": str(exc)}
    return {"status": "ready", "mode": "ollama"}


@router.get("/models")
async def list_models() -> dict:
    current = _current_mode()

    if current == "byok":
        # Return static BYOK model list
        return {
            "models": [
                {"id": "gpt-4o-mini", "provider": "openai", "name": "GPT-4o Mini", "default": True},
                {"id": "gpt-4o", "provider": "openai", "name": "GPT-4o", "default": False},
                {"id": "claude-sonnet-4-6", "provider": "anthropic", "name": "Claude Sonnet 4.6", "default": False},
            ]
        }

    # Ollama mode: query local models, filtering out cloud models
    try:
        tags = await mode_service.ollama_tags()
        mode_service.default_model(tags)
    except (httpx.HTTPError, httpx.ConnectError, httpx.TimeoutException):
        return {"models": [], "error": "ollama unreachable"}
    except ValueError as exc:
        return {"models": [], "error": str(exc)}
    names = mode_service.local_model_names(tags)
    models = [
        {"id": name, "provider": "ollama", "name": name, "default": i == 0}
        for i, name in enumerate(names)
    ]
    return {"models": models}
