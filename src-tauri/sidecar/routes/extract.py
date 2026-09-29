# ABOUTME: Synchronous text-extraction endpoint for uploaded documents.
# ABOUTME: Wraps services.extract: ExtractionError becomes a 422, an oversize upload a 413.

import anyio
from fastapi import APIRouter, HTTPException, UploadFile

from models.generated import ErrorResponse, ExtractDocumentResponse
from services.extract import ExtractionError, extract

router = APIRouter(tags=["extract"])

# Must equal MAX_UPLOAD_MB in src/api/sidecar.ts and src-tauri/src/lib.rs, which refuse larger files
# before reading or sending them; test_upload_limit_matches_the_{frontend,host} hold the three in step.
MAX_UPLOAD_MB = 50
MAX_UPLOAD_BYTES = MAX_UPLOAD_MB * 1024 * 1024


@router.post(
    "/extract",
    response_model=ExtractDocumentResponse,
    responses={413: {"model": ErrorResponse}, 422: {"model": ErrorResponse}},
)
async def extract_document(file: UploadFile) -> ExtractDocumentResponse:
    # Starlette spools uploads over 1 MB to disk, so checking the size here
    # keeps an oversize file out of memory.
    if file.size > MAX_UPLOAD_BYTES:
        raise HTTPException(
            status_code=413,
            detail=f"This file is larger than the {MAX_UPLOAD_MB} MB upload limit.",
        )
    data = await file.read()
    try:
        # Parsing is CPU-bound and can run for minutes; in a worker thread it
        # leaves the event loop free for /health, chat and job polling.
        text = await anyio.to_thread.run_sync(extract, file.filename or "", data)
    except ExtractionError as exc:
        raise HTTPException(status_code=422, detail=str(exc)) from exc
    return ExtractDocumentResponse(text=text)
