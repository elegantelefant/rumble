# ABOUTME: Health and readiness endpoints for the sidecar.
# ABOUTME: Reports status, checks Ollama reachability, and lists available models.

import os

import httpx
from fastapi import APIRouter

router = APIRouter(tags=["health"])

OLLAMA_BASE_URL = os.environ.get("OLLAMA_BASE_URL", "http://localhost:11434")


def _current_mode() -> str:
    """Determine operating mode from environment."""
    if os.environ.get("BYOK_API_KEY"):
        return "byok"
    return "ollama"


@router.get("/health")
async def health() -> dict:
    return {"status": "ok", "mode": _current_mode()}


@router.get("/ready")
async def ready() -> dict:
    mode = _current_mode()
    if mode == "byok":
        # BYOK mode: just check key is present
        return {"status": "ready", "mode": "byok"}

    # Ollama mode: check if Ollama is reachable
    try:
        async with httpx.AsyncClient(timeout=5.0) as client:
            resp = await client.get(f"{OLLAMA_BASE_URL}/api/tags")
            resp.raise_for_status()
            return {"status": "ready", "mode": "ollama"}
    except (httpx.HTTPError, httpx.ConnectError, httpx.TimeoutException):
        return {"status": "not_ready", "mode": "ollama", "error": "ollama unreachable"}


@router.get("/models")
async def list_models() -> dict:
    mode = _current_mode()

    if mode == "byok":
        # Return static BYOK model list
        return {
            "models": [
                {"id": "gpt-4o-mini", "provider": "openai", "name": "GPT-4o Mini", "default": True},
                {"id": "gpt-4o", "provider": "openai", "name": "GPT-4o", "default": False},
                {"id": "claude-sonnet-4-6", "provider": "anthropic", "name": "Claude Sonnet 4.6", "default": False},
            ]
        }

    # Ollama mode: query local models
    try:
        async with httpx.AsyncClient(timeout=5.0) as client:
            resp = await client.get(f"{OLLAMA_BASE_URL}/api/tags")
            resp.raise_for_status()
            data = resp.json()
            models = [
                {
                    "id": m["name"],
                    "provider": "ollama",
                    "name": m["name"],
                    "default": i == 0,
                }
                for i, m in enumerate(data.get("models", []))
            ]
            return {"models": models}
    except (httpx.HTTPError, httpx.ConnectError, httpx.TimeoutException):
        return {"models": []}
