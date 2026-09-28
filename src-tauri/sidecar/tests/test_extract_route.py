# ABOUTME: HTTP-level tests for POST /extract.
# ABOUTME: Exercises the real fixtures through the route, not just the service function.

from pathlib import Path

from docx import Document

FIXTURES = Path(__file__).parent / "fixtures"


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
