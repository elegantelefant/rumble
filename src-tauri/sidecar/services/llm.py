# ABOUTME: LLM service wrapping PydanticAI for Ollama and BYOK providers.
# ABOUTME: Provides send_message, stream_message for chat, and run_single_turn for stateless endpoints.

import os
from collections.abc import AsyncIterator

from pydantic_ai import Agent
from pydantic_ai.models.openai import OpenAIModel

from services import prompts

OLLAMA_BASE_URL = os.environ.get("OLLAMA_BASE_URL", "http://localhost:11434/v1")
OLLAMA_DEFAULT_MODEL = os.environ.get("OLLAMA_DEFAULT_MODEL", "llama3.2")


def _build_agent(
    system_prompt: str = prompts.CHAT,
    model_name: str | None = None,
    api_key: str | None = None,
) -> Agent:
    """Build a PydanticAI agent for the given provider config."""
    if api_key:
        model = OpenAIModel(
            model_name or "gpt-4o-mini",
            api_key=api_key,
        )
    else:
        model = OpenAIModel(
            model_name or OLLAMA_DEFAULT_MODEL,
            base_url=OLLAMA_BASE_URL,
            api_key="ollama",
        )
    return Agent(model=model, system_prompt=system_prompt)


async def send_message(
    messages: list[dict],
    system_prompt: str = prompts.CHAT,
    model_name: str | None = None,
    api_key: str | None = None,
) -> str:
    """Send a message and return the full response text."""
    agent = _build_agent(system_prompt, model_name, api_key)
    user_text = messages[-1]["content"] if messages else ""
    result = await agent.run(user_text)
    return result.output


async def stream_message(
    messages: list[dict],
    system_prompt: str = prompts.CHAT,
    model_name: str | None = None,
    api_key: str | None = None,
) -> AsyncIterator[str]:
    """Stream a message response, yielding text chunks."""
    agent = _build_agent(system_prompt, model_name, api_key)
    user_text = messages[-1]["content"] if messages else ""
    async with agent.run_stream(user_text) as stream:
        async for chunk in stream.stream_text(delta=True):
            yield chunk


async def run_single_turn(
    user_text: str,
    system_prompt: str,
    model_name: str | None = None,
    api_key: str | None = None,
) -> str:
    """Run a single-turn LLM call and return raw text. For stateless endpoints."""
    agent = _build_agent(system_prompt, model_name, api_key)
    result = await agent.run(user_text)
    return result.output
