# ABOUTME: Synchronous text-extraction endpoint for uploaded documents.
# ABOUTME: Wraps services.extract, turning ExtractionError into a 422 with its message.

from fastapi import APIRouter, HTTPException, UploadFile

from services.extract import ExtractionError, extract

router = APIRouter(tags=["extract"])


@router.post("/extract")
async def extract_document(file: UploadFile) -> dict:
    data = await file.read()
    try:
        text = extract(file.filename or "", data)
    except ExtractionError as exc:
        raise HTTPException(status_code=422, detail=str(exc)) from exc
    return {"text": text}
