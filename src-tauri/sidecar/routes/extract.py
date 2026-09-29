# ABOUTME: Synchronous text-extraction endpoint for uploaded documents.
# ABOUTME: Wraps services.extract, turning ExtractionError into a 422 with its message.

import anyio
from fastapi import APIRouter, HTTPException, UploadFile

from services.extract import ExtractionError, extract

router = APIRouter(tags=["extract"])


@router.post("/extract")
async def extract_document(file: UploadFile) -> dict:
    data = await file.read()
    try:
        # Parsing is CPU-bound and can run for minutes; in a worker thread it
        # leaves the event loop free for /health, chat and job polling.
        text = await anyio.to_thread.run_sync(extract, file.filename or "", data)
    except ExtractionError as exc:
        raise HTTPException(status_code=422, detail=str(exc)) from exc
    return {"text": text}
