# ABOUTME: Tests for document text extraction.
# ABOUTME: Builds real PDF and DOCX bytes rather than mocking the parsers.

import io
import zipfile
from pathlib import Path

import pytest
from docx import Document
from pypdf import PdfWriter

from services import extract as extract_service
from services.extract import DOCX_TOO_LARGE, ExtractionError, extract, extract_docx


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


def _padded_docx_bytes(filler: bytes) -> bytes:
    """A valid .docx with `filler` added to its body; repetitive filler deflates ~1000:1, as a crafted upload would."""
    source = zipfile.ZipFile(io.BytesIO(_docx_bytes(["Clause."])))
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as archive:
        for member in source.infolist():
            part = source.read(member.filename)
            if member.filename == "word/document.xml":
                part = part.replace(b"</w:body>", filler + b"</w:body>")
            archive.writestr(member.filename, part)
    return buf.getvalue()


def test_docx_over_the_uncompressed_limit_is_refused(monkeypatch):
    monkeypatch.setattr(extract_service, "MAX_DOCX_UNCOMPRESSED_BYTES", 1024 * 1024)
    data = _padded_docx_bytes(b"<!--" + b" " * 2 * 1024 * 1024 + b"-->")
    assert len(data) < 64 * 1024  # small on the wire, like the 232 KB original
    with pytest.raises(ExtractionError, match=DOCX_TOO_LARGE):
        extract_docx(data)


def test_docx_over_the_tag_limit_is_refused(monkeypatch):
    # python-docx's template alone has ~25,000 tags
    monkeypatch.setattr(extract_service, "MAX_DOCX_XML_TAGS", 50_000)
    data = _padded_docx_bytes(b"<w:p/>" * 40_000)  # 240 KB of XML, well under the byte limit
    with pytest.raises(ExtractionError, match=DOCX_TOO_LARGE):
        extract_docx(data)


def test_docx_under_both_limits_is_extracted(monkeypatch):
    monkeypatch.setattr(extract_service, "MAX_DOCX_UNCOMPRESSED_BYTES", 4 * 1024 * 1024)
    monkeypatch.setattr(extract_service, "MAX_DOCX_XML_TAGS", 100_000)
    data = _padded_docx_bytes(b"<!--" + b" " * 2 * 1024 * 1024 + b"-->" + b"<w:p/>" * 40_000)
    assert extract_docx(data) == "Clause."


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


def _user_password_pdf_bytes() -> bytes:
    writer = PdfWriter(clone_from=FIXTURES / "sample-text.pdf")
    writer.encrypt(user_password="open-sesame", algorithm="AES-256")
    buf = io.BytesIO()
    writer.write(buf)
    return buf.getvalue()


def test_owner_password_only_pdf_is_extracted():
    data = (FIXTURES / "sample-owner-password.pdf").read_bytes()
    assert "Lorem ipsum dolor sit amet" in extract("restricted.pdf", data)


def test_user_password_pdf_fails_honestly():
    with pytest.raises(ExtractionError, match="password protected"):
        extract("locked.pdf", _user_password_pdf_bytes())


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
