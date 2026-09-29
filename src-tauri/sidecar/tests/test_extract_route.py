# ABOUTME: HTTP-level tests for POST /extract.
# ABOUTME: Exercises the real fixtures through the route, not just the service function.

import asyncio
import threading
from pathlib import Path

from docx import Document

FIXTURES = Path(__file__).parent / "fixtures"
# How long the stand-in extraction would hold the event loop if it ran on it.
SLOW_EXTRACT_SECONDS = 2


def _docx_bytes(paragraphs: list[str]) -> bytes:
    import io

    doc = Document()
    for p in paragraphs:
        doc.add_paragraph(p)
    buf = io.BytesIO()
    doc.save(buf)
    return buf.getvalue()


async def test_extract_text_pdf_returns_200_with_text(client):
    data = (FIXTURES / "sample-text.pdf").read_bytes()
    resp = await client.post(
        "/extract", files={"file": ("sample-text.pdf", data, "application/pdf")}
    )
    assert resp.status_code == 200
    assert "Lorem ipsum dolor sit amet" in resp.json()["text"]


async def test_extract_docx_returns_200(client):
    data = _docx_bytes(["First clause.", "Second clause."])
    resp = await client.post(
        "/extract",
        files={
            "file": (
                "contract.docx",
                data,
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            )
        },
    )
    assert resp.status_code == 200
    assert "First clause." in resp.json()["text"]


async def test_extract_scanned_pdf_returns_422_with_message(client):
    data = (FIXTURES / "sample-scanned.pdf").read_bytes()
    resp = await client.post(
        "/extract", files={"file": ("sample-scanned.pdf", data, "application/pdf")}
    )
    assert resp.status_code == 422
    assert "scan or image-only" in resp.json()["detail"]


async def test_extract_unsupported_extension_returns_422(client):
    resp = await client.post(
        "/extract",
        files={"file": ("contract.doc", b"anything", "application/msword")},
    )
    assert resp.status_code == 422
    assert "Unsupported file type" in resp.json()["detail"]


async def test_extract_truncated_pdf_returns_422_in_user_language(client):
    data = (FIXTURES / "sample-text.pdf").read_bytes()[:200]
    resp = await client.post(
        "/extract", files={"file": ("contract.pdf", data, "application/pdf")}
    )
    assert resp.status_code == 422
    assert resp.json()["detail"] == (
        "This PDF could not be read. It may be damaged or not a real PDF."
    )


async def test_health_answers_while_an_extraction_is_in_flight(client, monkeypatch):
    started = threading.Event()
    release = threading.Event()

    def slow_extract(filename: str, data: bytes) -> str:
        started.set()
        release.wait(timeout=SLOW_EXTRACT_SECONDS)
        return "done"

    monkeypatch.setattr("routes.extract.extract", slow_extract)
    upload = asyncio.create_task(
        client.post("/extract", files={"file": ("notes.txt", b"x", "text/plain")})
    )
    while not started.is_set():
        await asyncio.sleep(0.01)

    health = await client.get("/health")
    extraction_still_running = not upload.done()
    release.set()
    await upload

    assert health.status_code == 200
    assert extraction_still_running
