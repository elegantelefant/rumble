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

    # Ollama mode: check if Ollama is reachable
    try:
        async with httpx.AsyncClient(timeout=5.0) as client:
            resp = await client.get(f"{mode_service.ollama_api_base()}/api/tags")
            resp.raise_for_status()
            return {"status": "ready", "mode": "ollama"}
    except (httpx.HTTPError, httpx.ConnectError, httpx.TimeoutException):
        return {"status": "not_ready", "mode": "ollama", "error": "ollama unreachable"}
    except ValueError as exc:
        return {"status": "not_ready", "mode": "ollama", "error": str(exc)}


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
        async with httpx.AsyncClient(timeout=5.0) as client:
            resp = await client.get(f"{mode_service.ollama_api_base()}/api/tags")
            resp.raise_for_status()
            data = resp.json()
            names = [m["name"] for m in data.get("models", []) if not mode_service.is_cloud_model(m["name"])]
            models = [
                {"id": name, "provider": "ollama", "name": name, "default": i == 0}
                for i, name in enumerate(names)
            ]
            return {"models": models}
    except (httpx.HTTPError, httpx.ConnectError, httpx.TimeoutException, ValueError):
        return {"models": []}
