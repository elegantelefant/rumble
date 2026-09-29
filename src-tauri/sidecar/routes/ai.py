# ABOUTME: Stateless AI endpoints — clarify, translate, summarise, chat_title.
# ABOUTME: Each endpoint takes a typed request, calls LLM once, and returns structured JSON.

import json
import logging
import re
import typing

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, TypeAdapter, ValidationError

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

logger = logging.getLogger(__name__)

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


def _list_item_type(annotation):
    """Return X if annotation is list[X] (optionally wrapped in Optional/Union), else None."""
    for candidate in (annotation, *typing.get_args(annotation)):
        if typing.get_origin(candidate) is list:
            args = typing.get_args(candidate)
            if args:
                return args[0]
    return None


def _list_item_submodel(annotation) -> type[BaseModel] | None:
    """Return X if annotation is list[X] (optionally wrapped in Optional/Union) and X is a BaseModel, else None."""
    item_type = _list_item_type(annotation)
    if item_type is not None and isinstance(item_type, type) and issubclass(item_type, BaseModel):
        return item_type
    return None


def _nested_submodel(annotation) -> type[BaseModel] | None:
    """Return X if annotation is X (optionally wrapped in Optional/Union) and X is a BaseModel, else None.

    Relies on isinstance(dict[str, Any], type) being False (true from Python 3.11 on),
    so a dict[str, Any] field like SearchResult.metadata is correctly skipped rather
    than raising. pyproject pins requires-python = ">=3.12", but backporting this
    helper to an older runtime would need re-checking that assumption or it would
    raise TypeError instead of the intended clean 502.
    """
    if isinstance(annotation, type) and issubclass(annotation, BaseModel):
        return annotation
    for arg in typing.get_args(annotation):
        if isinstance(arg, type) and issubclass(arg, BaseModel):
            return arg
    return None


def _drop_invalid_optional_fields(model_class: type[BaseModel], filtered: dict) -> None:
    """Drop optional fields whose value doesn't match the declared type, in place.

    An LLM inventing a value that doesn't fit an optional field (a ReviewIssue.kind
    we didn't enumerate, a warnings string where a list was expected) shouldn't 502
    the whole payload when the field already has a perfectly good default to fall
    back on. Required fields are left untouched, so a missing or wrong-typed
    required field still fails model_class(**filtered) and raises the usual 502.

    A list-valued field is validated item by item rather than as a whole: one
    malformed source shouldn't take every other, valid source down with it.
    """
    known_fields = model_class.model_fields
    for name in list(filtered):
        field = known_fields[name]
        if field.is_required():
            continue
        value = filtered[name]
        if isinstance(value, list):
            item_type = _list_item_type(field.annotation)
            if item_type is not None:
                kept = []
                for index, item in enumerate(value):
                    try:
                        TypeAdapter(item_type).validate_python(item)
                        kept.append(item)
                    except ValidationError:
                        logger.warning(
                            "Dropping invalid item %d in %s.%s: %r",
                            index, model_class.__name__, name, item,
                        )
                filtered[name] = kept
                continue
        try:
            TypeAdapter(field.annotation).validate_python(value)
        except ValidationError:
            del filtered[name]


def _filter_submodel_item(submodel: type[BaseModel], item):
    """Drop unknown keys from one list item, one level into any nested submodel field,
    and drop any optional field whose value doesn't match its declared type."""
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
    _drop_invalid_optional_fields(submodel, filtered_item)
    return filtered_item


def _safe_construct(model_class, data: dict):
    """Construct a Pydantic model, dropping unknown keys to survive extra='forbid'.

    Also filters one level into any list[SubModel] field (e.g. ReviewResponse.issues,
    ResearchResultResponse.sources), and one further level into a nested submodel
    field inside such an item (e.g. SearchResult.source): those submodels are
    extra='forbid' too, so an unexpected key inside them would otherwise still raise.
    Wrong-typed optional fields (top-level and, via _filter_submodel_item, inside
    list items) are dropped rather than failing the whole payload.

    Raises a clean 502 (rather than an unhandled 500) if the LLM's JSON is
    missing a required field or has a required field of the wrong type.
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
    _drop_invalid_optional_fields(model_class, filtered)
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
