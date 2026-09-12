# ABOUTME: Text extraction from uploaded documents for the review path.
# ABOUTME: Handles PDF and DOCX; fails honestly when there is no text layer.

import io

from docx import Document
from pypdf import PdfReader


class ExtractionError(Exception):
    """Raised when a document cannot be turned into usable text."""


def extract_pdf(data: bytes) -> str:
    try:
        reader = PdfReader(io.BytesIO(data))
    except Exception as exc:
        raise ExtractionError(f"could not read PDF: {exc}") from exc

    if reader.is_encrypted:
        raise ExtractionError("PDF is password protected")

    pages = [page.extract_text() or "" for page in reader.pages]
    text = "\n\n".join(p for p in pages if p.strip())

    if not text.strip():
        # A scanned document is images with no text layer. pypdf returns empty
        # strings rather than failing, so this is the only signal we get.
        raise ExtractionError(
            "No text found in this PDF. It may be a scan or image-only document."
        )

    return text


def extract_docx(data: bytes) -> str:
    try:
        document = Document(io.BytesIO(data))
    except Exception as exc:
        raise ExtractionError(f"could not read DOCX: {exc}") from exc

    parts = [p.text for p in document.paragraphs if p.text.strip()]

    for table in document.tables:
        for row in table.rows:
            cells = [c.text.strip() for c in row.cells if c.text.strip()]
            if cells:
                parts.append(" | ".join(cells))

    text = "\n\n".join(parts)

    if not text.strip():
        raise ExtractionError("No text found in this document.")

    return text


def extract(filename: str, data: bytes) -> str:
    """Dispatch on file extension. Plain text passes through unchanged."""
    lower = filename.lower()

    if lower.endswith(".pdf"):
        return extract_pdf(data)
    if lower.endswith(".docx"):
        return extract_docx(data)
    if lower.endswith((".txt", ".md")):
        try:
            return data.decode("utf-8")
        except UnicodeDecodeError as exc:
            raise ExtractionError("File is not valid UTF-8 text.") from exc

    raise ExtractionError(f"Unsupported file type: {filename}")
