# ABOUTME: Async job endpoints for heavy LLM tasks — draft, review, research.
# ABOUTME: POST creates a job and spawns background work; GET polls for results.

import json

from fastapi import APIRouter, HTTPException

from models.generated import (
    DraftRequest,
    JobCreatedResponse,
    JobResultResponse,
    ResearchRequest,
    ResearchResponse,
    ResearchResultResponse,
    ReviewRequest,
)
from routes.ai import _parse_llm_json, _safe_construct
from services import db, jobs, llm, prompts


def _parse_job_json(raw: str) -> dict:
    """Parse LLM output as JSON for background jobs. Raises ValueError, not HTTPException."""
    text = raw.strip()
    if text.startswith("```"):
        text = text.split("\n", 1)[1] if "\n" in text else text[3:]
        if text.rstrip().endswith("```"):
            text = text.rstrip()[:-3].strip()
    return json.loads(text)

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

    raw = await llm.run_single_turn(user_text, prompts.DRAFT, model_name=request.model or None)
    return _parse_job_json(raw)


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
    return await _poll_job(job_id)


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
    return await _poll_job(job_id)


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


@router.get("/research/{job_id}/result", response_model=ResearchResultResponse)
async def get_research_result(job_id: str) -> ResearchResultResponse:
    job = await db.get_job(job_id)
    if not job:
        raise HTTPException(status_code=404, detail="job not found")
    result_data = None
    if job.get("result"):
        try:
            result_data = json.loads(job["result"])
        except json.JSONDecodeError as exc:
            raise HTTPException(status_code=502, detail=f"malformed result: {exc}") from exc
    return _safe_construct(ResearchResultResponse, {
        "report_id": job["id"],
        "status": job["status"],
        "result": result_data.get("result") if result_data else None,
        "sources": result_data.get("sources") if result_data else None,
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
    return JobResultResponse(
        id=job["id"],
        status=job["status"],
        result=result_data,
    )
