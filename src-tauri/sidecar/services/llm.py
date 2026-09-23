# ABOUTME: LLM service wrapping PydanticAI for Ollama and BYOK providers.
# ABOUTME: Provides send_message, stream_message for chat, and run_single_turn for stateless endpoints.

import logging
import os
from collections.abc import AsyncIterator

import httpx
from pydantic_ai import Agent
from pydantic_ai.messages import ModelMessage, ModelRequest, ModelResponse, TextPart, UserPromptPart
from pydantic_ai.models.openai import OpenAIModel
from pydantic_ai.providers.openai import OpenAIProvider

from services import mode, prompts

logger = logging.getLogger(__name__)

OLLAMA_DEFAULT_MODEL = os.environ.get("OLLAMA_DEFAULT_MODEL", "llama3.2")

_resolved_ollama_model: str | None = None


async def _resolve_ollama_model() -> str:
    """Return the configured default model if available, else the first pulled model.

    Cloud-named models (services.mode.is_cloud_model) are filtered out of the
    candidate list first, so the "first pulled model" fallback can never
    silently land on one.
    """
    global _resolved_ollama_model
    if _resolved_ollama_model:
        return _resolved_ollama_model

    try:
        async with httpx.AsyncClient(timeout=5.0) as client:
            resp = await client.get(f"{mode.ollama_api_base()}/api/tags")
            resp.raise_for_status()
            models = [m["name"] for m in resp.json().get("models", []) if not mode.is_cloud_model(m["name"])]
    except (httpx.HTTPError, httpx.ConnectError, httpx.TimeoutException):
        logger.warning("Cannot reach Ollama to resolve model — using default %s", OLLAMA_DEFAULT_MODEL)
        return OLLAMA_DEFAULT_MODEL

    if OLLAMA_DEFAULT_MODEL in models:
        _resolved_ollama_model = OLLAMA_DEFAULT_MODEL
    elif models:
        _resolved_ollama_model = models[0]
        logger.info("Default model %s not found; using %s", OLLAMA_DEFAULT_MODEL, _resolved_ollama_model)
    else:
        _resolved_ollama_model = OLLAMA_DEFAULT_MODEL

    return _resolved_ollama_model


async def _build_agent(
    system_prompt: str = prompts.CHAT,
    model_name: str | None = None,
    api_key: str | None = None,
) -> Agent:
    """Build a PydanticAI agent for the given provider config.

    Branches on services.mode.current_mode(), never on whether a key happens
    to be present: a stray BYOK_API_KEY must not route an ollama-mode request
    to OpenAI, and a byok-mode request must not silently fall back to Ollama
    when no key is configured.
    """
    if mode.current_mode() == "byok":
        resolved_api_key = api_key or os.environ.get("BYOK_API_KEY")
        if not resolved_api_key:
            raise ValueError("byok mode requires an API key")
        provider = OpenAIProvider(api_key=resolved_api_key)
        model = OpenAIModel(model_name or "gpt-4o-mini", provider=provider)
    else:
        if model_name and mode.is_cloud_model(model_name):
            raise ValueError(f"refusing cloud model {model_name!r} in ollama mode")
        base_url = mode.ollama_openai_base_url()
        resolved = model_name or await _resolve_ollama_model()
        provider = OpenAIProvider(base_url=base_url, api_key="ollama")
        model = OpenAIModel(resolved, provider=provider)
    return Agent(model=model, system_prompt=system_prompt)


def _to_message_history(messages: list[dict]) -> list[ModelMessage]:
    """Convert DB messages to PydanticAI message history (excludes the last user message)."""
    history: list[ModelMessage] = []
    for msg in messages[:-1]:
        role = msg.get("role", "")
        content = msg.get("content", "")
        if role == "user":
            history.append(ModelRequest(parts=[UserPromptPart(content=content)]))
        elif content:
            history.append(ModelResponse(parts=[TextPart(content=content)]))
    return history


async def send_message(
    messages: list[dict],
    system_prompt: str = prompts.CHAT,
    model_name: str | None = None,
    api_key: str | None = None,
) -> str:
    """Send a message and return the full response text."""
    agent = await _build_agent(system_prompt, model_name, api_key)
    user_text = messages[-1].get("content", "") if messages else ""
    result = await agent.run(user_text, message_history=_to_message_history(messages))
    return result.output


async def stream_message(
    messages: list[dict],
    system_prompt: str = prompts.CHAT,
    model_name: str | None = None,
    api_key: str | None = None,
) -> AsyncIterator[str]:
    """Stream a message response, yielding text chunks."""
    agent = await _build_agent(system_prompt, model_name, api_key)
    user_text = messages[-1].get("content", "") if messages else ""
    async with agent.run_stream(user_text, message_history=_to_message_history(messages)) as stream:
        async for chunk in stream.stream_text(delta=True):
            yield chunk


async def run_single_turn(
    user_text: str,
    system_prompt: str,
    model_name: str | None = None,
    api_key: str | None = None,
) -> str:
    """Run a single-turn LLM call and return raw text. For stateless endpoints."""
    agent = await _build_agent(system_prompt, model_name, api_key)
    result = await agent.run(user_text)
    return result.output
