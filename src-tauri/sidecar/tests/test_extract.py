# ABOUTME: Tests for document text extraction.
# ABOUTME: Builds real PDF and DOCX bytes rather than mocking the parsers.

import io
from pathlib import Path

import pytest
from docx import Document

from services.extract import ExtractionError, extract, extract_docx


def _docx_bytes(paragraphs: list[str], table: list[list[str]] | None = None) -> bytes:
    doc = Document()
    for p in paragraphs:
        doc.add_paragraph(p)
    if table:
        t = doc.add_table(rows=len(table), cols=len(table[0]))
        for r, row in enumerate(table):
            for c, value in enumerate(row):
                t.cell(r, c).text = value
    buf = io.BytesIO()
    doc.save(buf)
    return buf.getvalue()


def test_extracts_docx_paragraphs():
    data = _docx_bytes(["First clause.", "Second clause."])
    text = extract_docx(data)
    assert "First clause." in text
    assert "Second clause." in text


def test_extracts_docx_tables():
    data = _docx_bytes(["Intro"], table=[["Party", "Obligation"], ["Acme", "Pay"]])
    text = extract_docx(data)
    assert "Party | Obligation" in text
    assert "Acme | Pay" in text


def test_empty_docx_raises():
    with pytest.raises(ExtractionError, match="No text found"):
        extract_docx(_docx_bytes([]))


def test_unreadable_docx_raises():
    with pytest.raises(ExtractionError, match="could not be read"):
        extract_docx(b"not a docx at all")


def test_unreadable_docx_message_hides_parser_internals():
    with pytest.raises(ExtractionError) as info:
        extract_docx(b"not a docx at all")
    assert "zip" not in str(info.value).lower()


FIXTURES = Path(__file__).parent / "fixtures"


def test_extracts_pdf_text_layer():
    data = (FIXTURES / "sample-text.pdf").read_bytes()
    text = extract("sample-text.pdf", data)
    assert "Lorem ipsum dolor sit amet" in text
    assert "commodo consequat" in text


def test_scanned_pdf_fails_honestly():
    # No text layer — pypdf returns empty strings rather than erroring, so the
    # emptiness check is the only signal that this is a scan.
    data = (FIXTURES / "sample-scanned.pdf").read_bytes()
    with pytest.raises(ExtractionError, match="scan or image-only"):
        extract("sample-scanned.pdf", data)


def test_unreadable_pdf_raises():
    with pytest.raises(ExtractionError, match="could not be read"):
        extract("contract.pdf", b"not a pdf at all")


def test_truncated_pdf_message_hides_parser_internals():
    data = (FIXTURES / "sample-text.pdf").read_bytes()
    with pytest.raises(ExtractionError) as info:
        extract("contract.pdf", data[:200])
    assert "Stream has ended" not in str(info.value)


def test_plain_text_passes_through():
    assert extract("notes.txt", b"clause one") == "clause one"
    assert extract("notes.md", b"# heading") == "# heading"


def test_non_utf8_text_raises():
    with pytest.raises(ExtractionError, match="not valid UTF-8"):
        extract("notes.txt", b"\xff\xfe invalid")


def test_unsupported_extension_raises():
    with pytest.raises(ExtractionError, match="Unsupported file type"):
        extract("contract.doc", b"anything")


def test_dispatch_is_case_insensitive():
    data = _docx_bytes(["Clause."])
    assert "Clause." in extract("CONTRACT.DOCX", data)
