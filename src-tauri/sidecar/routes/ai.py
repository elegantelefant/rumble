# ABOUTME: Stateless AI endpoints — clarify, translate, summarise, chat_title.
# ABOUTME: Each endpoint takes a typed request, calls LLM once, and returns structured JSON.

import json

from fastapi import APIRouter, HTTPException

from models.generated import (
    ChatTitleRequest,
    ChatTitleResponse,
    ClarifyRequest,
    ClarifyResponse,
    SummariseChatRequest,
    SummariseChatResponse,
    SummariseDocumentRequest,
    SummariseDocumentResponse,
    SummariseSearchRequest,
    SummariseSearchResponse,
    TranslateRequest,
    TranslateResponse,
)
from services import llm, prompts

router = APIRouter(tags=["ai"])


def _parse_llm_json(raw: str) -> dict:
    """Parse LLM output as JSON, stripping markdown fences if present."""
    text = raw.strip()
    if text.startswith("```"):
        # Strip opening fence (e.g. ```json)
        text = text.split("\n", 1)[1] if "\n" in text else text[3:]
        if text.rstrip().endswith("```"):
            text = text.rstrip()[:-3].strip()
    try:
        return json.loads(text)
    except (json.JSONDecodeError, ValueError) as exc:
        raise HTTPException(status_code=502, detail=f"LLM returned invalid JSON: {exc}") from exc


def _safe_construct(model_class, data: dict):
    """Construct a Pydantic model, dropping unknown keys to survive extra='forbid'."""
    known_fields = set(model_class.model_fields.keys())
    filtered = {k: v for k, v in data.items() if k in known_fields}
    return model_class(**filtered)


@router.post("/clarify", response_model=ClarifyResponse)
async def clarify(body: ClarifyRequest) -> ClarifyResponse:
    context_parts = []
    if body.context:
        context_parts.append(f"Context: {json.dumps(body.context)}")
    if body.document_type:
        context_parts.append(f"Document type: {body.document_type}")
    if body.drafting_style:
        context_parts.append(f"Style: {body.drafting_style}")
    user_text = "\n".join([body.ask, *context_parts]) if context_parts else body.ask

    raw = await llm.run_single_turn(user_text, prompts.CLARIFY)
    data = _parse_llm_json(raw)
    return _safe_construct(ClarifyResponse, data)


@router.post("/chat_title/generate", response_model=ChatTitleResponse)
async def generate_chat_title(body: ChatTitleRequest) -> ChatTitleResponse:
    user_text = "\n".join(body.messages)
    raw = await llm.run_single_turn(user_text, prompts.CHAT_TITLE)
    data = _parse_llm_json(raw)
    return _safe_construct(ChatTitleResponse, data)


@router.post("/translate", response_model=TranslateResponse)
async def translate(body: TranslateRequest) -> TranslateResponse:
    user_text = f"Translate to {body.target_lang}:\n{body.text}"
    raw = await llm.run_single_turn(user_text, prompts.TRANSLATE)
    data = _parse_llm_json(raw)
    return _safe_construct(TranslateResponse, data)


@router.post("/summarise/document", response_model=SummariseDocumentResponse)
async def summarise_document(body: SummariseDocumentRequest) -> SummariseDocumentResponse:
    style_hint = f" (style: {body.style})" if body.style else ""
    user_text = f"Summarise this document{style_hint}:\n{body.text}"
    raw = await llm.run_single_turn(user_text, prompts.SUMMARISE_DOCUMENT)
    data = _parse_llm_json(raw)
    return _safe_construct(SummariseDocumentResponse, data)


@router.post("/summarise/chat", response_model=SummariseChatResponse)
async def summarise_chat(body: SummariseChatRequest) -> SummariseChatResponse:
    msgs = body.messages or []
    formatted = "\n".join(f"{m.get('role', '?')}: {m.get('content', '')}" for m in msgs)
    style_hint = f" (style: {body.style})" if body.style else ""
    user_text = f"Summarise this chat{style_hint}:\n{formatted}"
    raw = await llm.run_single_turn(user_text, prompts.SUMMARISE_CHAT)
    data = _parse_llm_json(raw)
    return _safe_construct(SummariseChatResponse, data)


@router.post("/summarise/search", response_model=SummariseSearchResponse)
async def summarise_search(body: SummariseSearchRequest) -> SummariseSearchResponse:
    results_text = "\n".join(f"- {r.title}: {r.id}" for r in body.results)
    user_text = f"Query: {body.query}\nResults:\n{results_text}"
    raw = await llm.run_single_turn(user_text, prompts.SUMMARISE_SEARCH)
    data = _parse_llm_json(raw)
    return _safe_construct(SummariseSearchResponse, data)
