# ABOUTME: Unit tests for _parse_llm_json — verifies robust LLM output parsing.
# ABOUTME: Covers raw JSON, fenced JSON, preamble text, garbage input, and repair (#90).

from pathlib import Path

import pytest
from fastapi import HTTPException

from routes.ai import _parse_llm_json

FIXTURES = Path(__file__).parent / "fixtures"


def _fixture(name: str) -> str:
    return (FIXTURES / name).read_text()


def test_raw_json():
    assert _parse_llm_json('{"key": "value"}') == {"key": "value"}


def test_fenced_with_json_tag():
    raw = '```json\n{"key": "value"}\n```'
    assert _parse_llm_json(raw) == {"key": "value"}


def test_fenced_without_tag():
    raw = '```\n{"key": "value"}\n```'
    assert _parse_llm_json(raw) == {"key": "value"}


def test_trailing_newline_after_fence():
    raw = '```json\n{"key": "value"}\n```\n'
    assert _parse_llm_json(raw) == {"key": "value"}


def test_preamble_text_before_fence():
    raw = 'Here is the result:\n```json\n{"key": "value"}\n```'
    assert _parse_llm_json(raw) == {"key": "value"}


def test_preamble_and_trailing_text():
    raw = 'Sure!\n```json\n{"key": "value"}\n```\nHope that helps!'
    assert _parse_llm_json(raw) == {"key": "value"}


def test_fenced_json_tag_uppercase():
    raw = '```JSON\n{"key": "value"}\n```'
    assert _parse_llm_json(raw) == {"key": "value"}


def test_garbage_returns_502():
    with pytest.raises(HTTPException) as exc_info:
        _parse_llm_json("this is not json at all")
    assert exc_info.value.status_code == 502
    assert "unparseable" in exc_info.value.detail


def test_empty_string_returns_502():
    with pytest.raises(HTTPException) as exc_info:
        _parse_llm_json("")
    assert exc_info.value.status_code == 502


def test_nested_json():
    raw = '```json\n{"summary": "test", "items": [1, 2, 3]}\n```'
    assert _parse_llm_json(raw) == {"summary": "test", "items": [1, 2, 3]}


def test_whitespace_padding():
    raw = '  \n  {"key": "value"}  \n  '
    assert _parse_llm_json(raw) == {"key": "value"}


def test_think_tags_before_json():
    raw = '<think>I should generate a short title</think>\n{"title": "Force Majeure"}'
    assert _parse_llm_json(raw) == {"title": "Force Majeure"}


def test_think_tags_before_fenced_json():
    raw = '<think>Let me think about this</think>\n```json\n{"key": "value"}\n```'
    assert _parse_llm_json(raw) == {"key": "value"}


def test_think_tags_multiline():
    raw = '<think>\nStep 1: parse the request\nStep 2: generate title\n</think>\n{"title": "Test"}'
    assert _parse_llm_json(raw) == {"title": "Test"}


def test_think_tags_only_returns_502():
    with pytest.raises(HTTPException) as exc_info:
        _parse_llm_json("<think>thinking...</think>")
    assert exc_info.value.status_code == 502


# --- Repair fallback (#90) -- fixtures are real, saved model outputs from
# .scratch/90-parse-experiments/ (the NDA reviewed is a fictional sample,
# so these are safe to commit). Repair is opt-in (allow_repair=True), scoped
# to the review job path (#92 review) -- every test below passes it
# explicitly, since that's the only caller that ever does. ---

def test_repairs_a_trailing_comma_from_a_real_review_failure():
    raw = _fixture("review-parse-trailing-comma.txt")
    result = _parse_llm_json(raw, allow_repair=True)
    assert result["summary"]
    assert len(result["issues"]) == 5


def test_repairs_a_control_character_from_a_real_review_failure():
    raw = _fixture("review-parse-control-char.txt")
    result = _parse_llm_json(raw, allow_repair=True)
    assert result["summary"]
    assert len(result["issues"]) == 4


def test_repairs_a_missing_comma_delimiter_from_a_real_review_failure():
    raw = _fixture("review-parse-missing-comma.txt")
    result = _parse_llm_json(raw, allow_repair=True)
    assert result["summary"]
    assert len(result["issues"]) == 5


def test_repair_is_scoped_to_callers_that_opt_in():
    """Every other _parse_llm_json caller (translate, clarify, chat_title,
    the three summaries, and draft/research via _parse_job_json) doesn't
    pass allow_repair, so a malformed-but-mechanically-repairable output
    there 502s honestly instead of silently returning a repaired result --
    even against this fixture, which IS repairable with allow_repair=True
    (the test above)."""
    raw = _fixture("review-parse-trailing-comma.txt")
    with pytest.raises(HTTPException) as exc_info:
        _parse_llm_json(raw)
    assert exc_info.value.status_code == 502


def test_truncated_output_skips_repair_and_raises_502_instead_of_being_repaired(caplog):
    """Output with no closing brace/bracket means the model ran out of
    tokens mid-structure, not that it made a syntax slip -- json-repair
    would still produce *valid* JSON by inventing a close, not by
    recovering content that was never generated. This must be treated as
    truncated and left for the caller's retry, not repaired into a result
    with a half-written message."""
    raw = _fixture("review-parse-truncated.txt")
    with caplog.at_level("WARNING"):
        with pytest.raises(HTTPException) as exc_info:
            _parse_llm_json(raw, allow_repair=True)
    assert exc_info.value.status_code == 502
    assert not any("Repaired malformed LLM JSON" in r.message for r in caplog.records)


def test_repair_exception_logs_only_the_exception_type_never_the_message(monkeypatch, caplog):
    """`except Exception: repaired = None` used to swallow a json-repair
    failure silently; it must now log the exception's type (so a library
    regression shows up) but never its message or the content, either of
    which is the user's own document text."""
    import routes.ai as ai_module

    def _boom(*args, **kwargs):
        raise RuntimeError("some-document-specific-text-that-must-never-be-logged")

    monkeypatch.setattr(ai_module.json_repair, "loads", _boom)

    raw = _fixture("review-parse-trailing-comma.txt")
    with caplog.at_level("WARNING"):
        with pytest.raises(HTTPException):
            _parse_llm_json(raw, allow_repair=True)

    messages = [r.message for r in caplog.records]
    assert any("RuntimeError" in m for m in messages)
    assert not any("some-document-specific-text-that-must-never-be-logged" in m for m in messages)


def test_repair_logs_a_warning_but_never_the_documents_own_content(caplog):
    raw = _fixture("review-parse-trailing-comma.txt")
    with caplog.at_level("WARNING"):
        _parse_llm_json(raw, allow_repair=True)
    messages = [r.message for r in caplog.records]
    assert any("Repaired malformed LLM JSON" in m for m in messages)
    # The fixture's own text (the review's summary/issue content) must never
    # appear in the log -- it's the user's own document, same discipline as
    # the draft-retry log (rumble#46).
    assert not any("Non-Disclosure Agreement" in m for m in messages)


def test_a_failure_repair_cannot_fix_still_raises_502():
    """Not every malformed output is mechanically repairable -- this fixture
    is genuinely garbled structure (the model closed the issues array early,
    then wrote a second, separately-malformed fragment right after it), not
    a syntax slip."""
    raw = _fixture("review-parse-unrepairable.txt")
    with pytest.raises(HTTPException) as exc_info:
        _parse_llm_json(raw, allow_repair=True)
    assert exc_info.value.status_code == 502


def test_a_repaired_bare_array_is_not_accepted_as_a_json_object_response():
    """json_repair happily repairs the trailing comma in "[1, 2, 3,]" into a
    valid list -- but every endpoint here expects an object (summary/issues,
    draft/warnings, etc.), so a repaired non-dict must still be treated as a
    failure, not silently returned as if it were a valid response. (Ends in
    "]", so this exercises the non-dict rejection, not the truncation skip.)"""
    with pytest.raises(HTTPException) as exc_info:
        _parse_llm_json("[1, 2, 3,]", allow_repair=True)
    assert exc_info.value.status_code == 502
