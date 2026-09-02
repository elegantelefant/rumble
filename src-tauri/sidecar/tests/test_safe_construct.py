# ABOUTME: Unit tests for _safe_construct's field filtering and validation error handling.
# ABOUTME: Covers extra-field dropping, missing required fields, and wrong-type values.
import pytest
from fastapi import HTTPException
from pydantic import BaseModel

from routes.ai import _safe_construct


class _SampleModel(BaseModel):
    model_config = {"extra": "forbid"}
    required_field: str
    optional_field: str | None = None


def test_drops_unknown_keys():
    result = _safe_construct(
        _SampleModel, {"required_field": "value", "extra_key": "should be dropped"}
    )
    assert result.required_field == "value"
    assert not hasattr(result, "extra_key")


def test_keeps_known_optional_field():
    result = _safe_construct(
        _SampleModel, {"required_field": "value", "optional_field": "present"}
    )
    assert result.optional_field == "present"


def test_missing_required_field_raises_502():
    with pytest.raises(HTTPException) as exc_info:
        _safe_construct(_SampleModel, {"optional_field": "no required field here"})
    assert exc_info.value.status_code == 502
    assert "expected shape" in exc_info.value.detail


def test_wrong_type_raises_502():
    with pytest.raises(HTTPException) as exc_info:
        _safe_construct(_SampleModel, {"required_field": {"not": "a string"}})
    assert exc_info.value.status_code == 502
