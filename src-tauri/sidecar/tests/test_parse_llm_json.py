# ABOUTME: Unit tests for _parse_llm_json — verifies robust LLM output parsing.
# ABOUTME: Covers raw JSON, fenced JSON, preamble text, and garbage input.

import pytest
from fastapi import HTTPException

from routes.ai import _parse_llm_json


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
