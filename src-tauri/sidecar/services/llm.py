# ABOUTME: LLM service wrapping PydanticAI for Ollama and BYOK providers.
# ABOUTME: Provides send_message, stream_message for chat, and run_single_turn for stateless endpoints.

import logging
import os
from collections.abc import AsyncIterator

from pydantic_ai import Agent
from pydantic_ai.messages import ModelMessage, ModelRequest, ModelResponse, TextPart, UserPromptPart
from pydantic_ai.models.openai import OpenAIModel
from pydantic_ai.providers.openai import OpenAIProvider

from services import mode, prompts

logger = logging.getLogger(__name__)

_resolved_ollama_model: str | None = None


async def _resolve_ollama_model(model_name: str | None = None) -> str:
    """Return the /api/tags name of a local model: the requested one, else OLLAMA_DEFAULT_MODEL, else the first pulled.

    Every candidate, the configured default included, is checked against
    Ollama's own metadata (services.mode.local_model). A requested name that
    isn't a pulled local model is refused rather than passed through: what an
    unlisted name resolves to is Ollama's decision, so it can't be verified local.
    """
    if model_name:
        resolved = mode.local_model(model_name, await mode.ollama_tags())
        if resolved is None:
            raise ValueError(f"model {model_name!r} is not a pulled local Ollama model")
        return resolved

    global _resolved_ollama_model
    if _resolved_ollama_model:
        return _resolved_ollama_model

    tags = await mode.ollama_tags()
    resolved = mode.default_model(tags)
    if resolved is None:
        local = mode.local_model_names(tags)
        if not local:
            raise ValueError("no local Ollama model is pulled")
        resolved = local[0]
        logger.info("Default model not pulled; using %s", resolved)
    _resolved_ollama_model = resolved
    return resolved


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
        resolved = await _resolve_ollama_model(model_name)
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
