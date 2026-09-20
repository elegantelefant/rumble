# ABOUTME: Stateless AI endpoints — clarify, translate, summarise, chat_title.
# ABOUTME: Each endpoint takes a typed request, calls LLM once, and returns structured JSON.

import json
import re
import typing
from enum import Enum

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, ValidationError

from models.generated import (
    ChatTitleRequest,
    ChatTitleResponse,
    ClarifyRequest,
    ClarifyResponse,
    ReviewIssue,
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
    """Parse LLM output as JSON, stripping thinking tags and markdown fences."""
    text = raw.strip()
    # Strip <think>...</think> blocks (reasoning models like DeepSeek-R1)
    text = re.sub(r"<think>.*?</think>", "", text, flags=re.DOTALL).strip()
    # Strip markdown fence if present (handles preamble text before fence)
    if "```" in text:
        parts = text.split("```")
        for part in parts:
            candidate = part.strip()
            # Remove optional language tag (e.g. "json\n")
            if candidate.lower().startswith("json"):
                candidate = candidate.split("\n", 1)[-1].strip()
            if candidate.startswith("{"):
                text = candidate
                break
    try:
        return json.loads(text)
    except (json.JSONDecodeError, ValueError) as exc:
        raise HTTPException(status_code=502, detail=f"LLM returned unparseable output: {exc}") from exc


def _list_item_submodel(annotation) -> type[BaseModel] | None:
    """Return X if annotation is list[X] (optionally wrapped in Optional/Union), else None."""
    for candidate in (annotation, *typing.get_args(annotation)):
        if typing.get_origin(candidate) is list:
            args = typing.get_args(candidate)
            if args and isinstance(args[0], type) and issubclass(args[0], BaseModel):
                return args[0]
    return None


def _enum_type(annotation) -> type[Enum] | None:
    """Return the Enum subclass in annotation (optionally wrapped in Optional), else None."""
    if isinstance(annotation, type) and issubclass(annotation, Enum):
        return annotation
    for arg in typing.get_args(annotation):
        if isinstance(arg, type) and issubclass(arg, Enum):
            return arg
    return None


def _nested_submodel(annotation) -> type[BaseModel] | None:
    """Return X if annotation is X (optionally wrapped in Optional/Union) and X is a BaseModel, else None."""
    if isinstance(annotation, type) and issubclass(annotation, BaseModel):
        return annotation
    for arg in typing.get_args(annotation):
        if isinstance(arg, type) and issubclass(arg, BaseModel):
            return arg
    return None


def _filter_submodel_item(submodel: type[BaseModel], item):
    """Drop unknown keys from one list item, one level into any nested submodel field, and special-case ReviewIssue.kind."""
    if not isinstance(item, dict):
        return item
    known_fields = submodel.model_fields
    filtered_item = {k: v for k, v in item.items() if k in known_fields}
    for name, field in known_fields.items():
        if not isinstance(filtered_item.get(name), dict):
            continue
        nested = _nested_submodel(field.annotation)
        if nested is not None:
            nested_fields = nested.model_fields
            filtered_item[name] = {k: v for k, v in filtered_item[name].items() if k in nested_fields}
    if submodel is ReviewIssue and "kind" in filtered_item:
        # kind is a StrEnum whose default ('other') exists precisely for this case: the
        # LLM inventing a category we didn't enumerate should fall back to it, not 502.
        # No other list-item field has a default that makes an unrecognised value safe
        # to paper over, so this stays specific to ReviewIssue.kind rather than generic.
        enum_cls = _enum_type(known_fields["kind"].annotation)
        value = filtered_item["kind"]
        if enum_cls and not isinstance(value, enum_cls):
            try:
                enum_cls(value)
            except ValueError:
                filtered_item["kind"] = known_fields["kind"].default
    return filtered_item


def _safe_construct(model_class, data: dict):
    """Construct a Pydantic model, dropping unknown keys to survive extra='forbid'.

    Also filters one level into any list[SubModel] field (e.g. ReviewResponse.issues,
    ResearchResultResponse.sources), and one further level into a nested submodel
    field inside such an item (e.g. SearchResult.source): those submodels are
    extra='forbid' too, so an unexpected key inside them would otherwise still raise.

    Raises a clean 502 (rather than an unhandled 500) if the LLM's JSON is
    missing required fields or has values of the wrong type.
    """
    known_fields = model_class.model_fields
    filtered = {k: v for k, v in data.items() if k in known_fields}
    for name, value in filtered.items():
        if not isinstance(value, list):
            continue
        submodel = _list_item_submodel(known_fields[name].annotation)
        if submodel is not None:
            # Rebinding an existing key doesn't resize the dict, so this is safe during iteration.
            filtered[name] = [_filter_submodel_item(submodel, item) for item in value]
    try:
        return model_class(**filtered)
    except ValidationError as exc:
        raise HTTPException(
            status_code=502,
            detail=f"LLM response did not match expected shape: {exc}",
        ) from exc


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

    try:
        raw = await llm.run_single_turn(user_text, prompts.CLARIFY)
    except Exception as exc:
        raise HTTPException(status_code=502, detail=f"LLM error: {exc}") from exc
    data = _parse_llm_json(raw)
    return _safe_construct(ClarifyResponse, data)


@router.post("/chat_title/generate", response_model=ChatTitleResponse)
async def generate_chat_title(body: ChatTitleRequest) -> ChatTitleResponse:
    user_text = "\n".join(body.messages)
    try:
        raw = await llm.run_single_turn(user_text, prompts.CHAT_TITLE)
    except Exception as exc:
        raise HTTPException(status_code=502, detail=f"LLM error: {exc}") from exc
    data = _parse_llm_json(raw)
    return _safe_construct(ChatTitleResponse, data)


@router.post("/translate", response_model=TranslateResponse)
async def translate(body: TranslateRequest) -> TranslateResponse:
    if not re.match(r"^[a-zA-Z]{2,10}(-[a-zA-Z]{2,10})?$", body.target_lang):
        raise HTTPException(status_code=422, detail="invalid target_lang format")
    user_text = f"Translate to {body.target_lang}:\n{body.text}"
    try:
        raw = await llm.run_single_turn(user_text, prompts.TRANSLATE)
    except Exception as exc:
        raise HTTPException(status_code=502, detail=f"LLM error: {exc}") from exc
    data = _parse_llm_json(raw)
    return _safe_construct(TranslateResponse, data)


@router.post("/summarise/document", response_model=SummariseDocumentResponse)
async def summarise_document(body: SummariseDocumentRequest) -> SummariseDocumentResponse:
    style_hint = f" (style: {body.style})" if body.style else ""
    user_text = f"Summarise this document{style_hint}:\n{body.text}"
    try:
        raw = await llm.run_single_turn(user_text, prompts.SUMMARISE_DOCUMENT)
    except Exception as exc:
        raise HTTPException(status_code=502, detail=f"LLM error: {exc}") from exc
    data = _parse_llm_json(raw)
    return _safe_construct(SummariseDocumentResponse, data)


@router.post("/summarise/chat", response_model=SummariseChatResponse)
async def summarise_chat(body: SummariseChatRequest) -> SummariseChatResponse:
    msgs = body.messages or []
    formatted = "\n".join(f"{m.get('role', '?')}: {m.get('content', '')}" for m in msgs)
    style_hint = f" (style: {body.style})" if body.style else ""
    user_text = f"Summarise this chat{style_hint}:\n{formatted}"
    try:
        raw = await llm.run_single_turn(user_text, prompts.SUMMARISE_CHAT)
    except Exception as exc:
        raise HTTPException(status_code=502, detail=f"LLM error: {exc}") from exc
    data = _parse_llm_json(raw)
    return _safe_construct(SummariseChatResponse, data)


@router.post("/summarise/search", response_model=SummariseSearchResponse)
async def summarise_search(body: SummariseSearchRequest) -> SummariseSearchResponse:
    results_text = "\n".join(f"- {r.title}: {r.id}" for r in body.results)
    user_text = f"Query: {body.query}\nResults:\n{results_text}"
    try:
        raw = await llm.run_single_turn(user_text, prompts.SUMMARISE_SEARCH)
    except Exception as exc:
        raise HTTPException(status_code=502, detail=f"LLM error: {exc}") from exc
    data = _parse_llm_json(raw)
    return _safe_construct(SummariseSearchResponse, data)
