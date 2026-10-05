# ABOUTME: Async job endpoints for heavy LLM tasks — draft, review, research.
# ABOUTME: POST creates a job and spawns background work; GET polls for results.

import json
import logging

from fastapi import APIRouter, HTTPException

logger = logging.getLogger(__name__)

from models.generated import (
    DraftRequest,
    DraftResponse,
    JobCreatedResponse,
    JobResultResponse,
    ResearchRequest,
    ResearchResponse,
    ResearchResultResponse,
    ReviewRequest,
    ReviewResponse,
)
from routes.ai import _parse_llm_json, _safe_construct
from services import db, draft_substitution, jobs, llm, prompts


def _parse_job_json(raw: str) -> dict:
    """Parse LLM output as JSON for background jobs. Raises ValueError, not HTTPException."""
    from fastapi import HTTPException
    try:
        return _parse_llm_json(raw)
    except HTTPException as exc:
        raise ValueError(exc.detail) from exc

router = APIRouter(tags=["jobs"])


# --- Draft ---

async def _run_draft(request: DraftRequest) -> dict:
    context_parts = []
    if request.document_type:
        context_parts.append(f"Document type: {request.document_type}")
    if request.drafting_style:
        context_parts.append(f"Style: {request.drafting_style}")
    if request.parties:
        context_parts.append(f"Parties: {json.dumps(request.parties)}")
    if request.document_terms:
        context_parts.append(f"Terms: {json.dumps(request.document_terms)}")
    user_text = "\n".join([request.prompt, *context_parts]) if context_parts else request.prompt

    # Plain prose, not JSON (rumble#46, services/prompts.py's DRAFT comment):
    # the model's raw text IS the draft, substituted against the form's own
    # fields rather than parsed as a contract the model has to get exactly
    # right. One retry if a provided value is still missing afterwards --
    # not retried indefinitely, since a model that omits it once tends to
    # omit it again, and the job should still resolve.
    fields = request.fields or []
    raw = await llm.run_single_turn(user_text, prompts.DRAFT, model_name=request.model or None)
    draft, _ = draft_substitution.apply(raw, fields)

    if fields:
        presence = draft_substitution.all_present(draft, fields)
        missing_keys = [key for key, present in presence.items() if not present]
        if missing_keys:
            # warning, not info: uvicorn's log_level="warning" (main.py) sets
            # the root logger's effective level, so info() here would be
            # silently dropped and never visible without changing the
            # deployed log level.
            # Keys only, never values: a value is the user's own document
            # content (an employee's name, a client's address), and a log
            # line is not where that belongs.
            logger.warning("Draft retry: missing field(s) %s after substitution", missing_keys)
            raw = await llm.run_single_turn(user_text, prompts.DRAFT, model_name=request.model or None)
            draft, _ = draft_substitution.apply(raw, fields)

    return {"draft": draft, "warnings": []}


@router.post("/draft", response_model=JobCreatedResponse)
async def create_draft(body: DraftRequest) -> JobCreatedResponse:
    job = await db.create_job("draft", body.model_dump_json())
    await jobs.start_job(job["id"], _run_draft(body))
    return JobCreatedResponse(
        job_id=job["id"],
        status="queued",
        poll_url=f"/draft/{job['id']}/result",
    )


@router.get("/draft/{job_id}/result", response_model=JobResultResponse)
async def get_draft_result(job_id: str) -> JobResultResponse:
    response = await _poll_job(job_id)
    if response.status == "completed" and response.result is not None:
        # A completed job's result is persisted in SQLite with no re-run path, so
        # a stored result missing a required field (or {}) 502s here on every poll
        # from now on. That's deliberate: the result was garbage when it was saved,
        # and nothing here could fix it in place.
        response.result = _safe_construct(DraftResponse, response.result).model_dump()
    return response


# --- Review ---

async def _run_review(request: ReviewRequest) -> dict:
    context_parts = []
    if request.instructions:
        context_parts.append(f"Instructions: {request.instructions}")
    if request.context:
        context_parts.append(f"Context: {json.dumps(request.context)}")
    user_text = "\n".join([request.text, *context_parts]) if context_parts else request.text

    raw = await llm.run_single_turn(user_text, prompts.REVIEW)
    return _parse_job_json(raw)


@router.post("/review", response_model=JobCreatedResponse)
async def create_review(body: ReviewRequest) -> JobCreatedResponse:
    job = await db.create_job("review", body.model_dump_json())
    await jobs.start_job(job["id"], _run_review(body))
    return JobCreatedResponse(
        job_id=job["id"],
        status="queued",
        poll_url=f"/review/{job['id']}/result",
    )


@router.get("/review/{job_id}/result", response_model=JobResultResponse)
async def get_review_result(job_id: str) -> JobResultResponse:
    response = await _poll_job(job_id)
    if response.status == "completed" and response.result is not None:
        # Raw passthrough, unlike get_draft_result above: DocumentReviewView reads
        # issue.severity/issue.description, which aren't ReviewIssue fields
        # (kind/message/location/suggestion), so filtering to the constructed
        # model would strip what it renders. Flip this to match get_draft_result
        # once the view is updated to match the model (rumble#49). Same
        # permanent-502 caveat as get_draft_result applies here too.
        _safe_construct(ReviewResponse, response.result)
    return response


# --- Research ---

async def _run_research(request: ResearchRequest) -> dict:
    context_parts = []
    if request.constraints:
        context_parts.append(f"Constraints: {json.dumps(request.constraints)}")
    if request.guidelines:
        context_parts.append(f"Guidelines: {request.guidelines}")
    if request.primary_source:
        context_parts.append(f"Primary source: {request.primary_source}")
    user_text = "\n".join([request.question, *context_parts]) if context_parts else request.question

    raw = await llm.run_single_turn(user_text, prompts.RESEARCH, model_name=request.model or None)
    return _parse_job_json(raw)


@router.post("/research", response_model=ResearchResponse)
async def create_research(body: ResearchRequest) -> ResearchResponse:
    job = await db.create_job("research", body.model_dump_json())
    await jobs.start_job(job["id"], _run_research(body))
    return ResearchResponse(report_id=job["id"], status="queued")


@router.get("/research/{job_id}")
async def get_research_status(job_id: str) -> dict:
    job = await db.get_job(job_id)
    if not job:
        raise HTTPException(status_code=404, detail="job not found")
    return {
        "id": job["id"],
        "type": job["type"],
        "status": job["status"],
        "created_at": job["created_at"],
        "started_at": job.get("started_at"),
        "completed_at": job.get("completed_at"),
        "error": job.get("error"),
    }


def _normalize_sources(raw_sources: list | None) -> list[dict] | None:
    """Convert LLM source output into SearchResult-compatible dicts."""
    if not raw_sources:
        return None
    normalized = []
    for i, src in enumerate(raw_sources):
        if isinstance(src, str):
            normalized.append({"id": f"src-{i}", "title": src})
        elif isinstance(src, dict):
            if "id" not in src:
                src["id"] = f"src-{i}"
            if "title" not in src:
                src["title"] = src.get("name", src.get("url", f"Source {i+1}"))
            normalized.append(src)
    return normalized


@router.get("/research/{job_id}/result", response_model=ResearchResultResponse)
async def get_research_result(job_id: str) -> ResearchResultResponse:
    job = await db.get_job(job_id)
    if not job:
        raise HTTPException(status_code=404, detail="job not found")
    if job["status"] == "failed":
        error_msg = job.get("error") or "unknown error"
        return _safe_construct(ResearchResultResponse, {
            "report_id": job["id"],
            "status": job["status"],
            "result": error_msg,
            "sources": None,
        })
    result_data = None
    if job.get("result"):
        try:
            result_data = json.loads(job["result"])
        except json.JSONDecodeError as exc:
            raise HTTPException(status_code=502, detail=f"malformed result: {exc}") from exc
    sources = _normalize_sources(result_data.get("sources")) if result_data else None
    return _safe_construct(ResearchResultResponse, {
        "report_id": job["id"],
        "status": job["status"],
        "result": result_data.get("result") if result_data else None,
        "sources": sources,
    })


# --- Shared poll helper ---

async def _poll_job(job_id: str) -> JobResultResponse:
    job = await db.get_job(job_id)
    if not job:
        raise HTTPException(status_code=404, detail="job not found")
    result_data = None
    if job.get("result"):
        try:
            result_data = json.loads(job["result"])
        except json.JSONDecodeError as exc:
            raise HTTPException(status_code=502, detail=f"malformed result: {exc}") from exc
    elif job["status"] == "failed" and job.get("error"):
        result_data = {"error": job["error"]}
    return JobResultResponse(
        id=job["id"],
        status=job["status"],
        result=result_data,
    )
