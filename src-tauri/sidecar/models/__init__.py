# ABOUTME: Pydantic models package for rumble sidecar.
# ABOUTME: Contains generated models from openapi.json and any local overrides.

from typing import Any

from pydantic import BaseModel, ConfigDict, Field

from models.generated import SearchResult, Status

# Local overrides for sidecar-owned route contracts that the cloud spec no longer
# fits: 0.4.0 dropped ResearchResultResponse / ChatCreateResponse entirely, and
# reshaped JobResultResponse.result into a strict jobType-discriminated union that
# the sidecar's free-form LLM results don't match. The sidecar still serves these
# routes, so it owns these models here. (Other job models like JobCreatedResponse /
# ResearchResponse still come straight from models.generated.)


class ChatCreateResponse(BaseModel):
    """Response from the sidecar's POST /chats route."""

    model_config = ConfigDict(extra="forbid")

    id: str = Field(..., title="Id")


class JobResultResponse(BaseModel):
    """Sidecar job result. result is a free-form dict of LLM output, unlike the
    cloud's strict typed union which the sidecar's outputs don't carry."""

    model_config = ConfigDict(extra="forbid")

    id: str = Field(..., title="Id")
    status: Status
    result: dict[str, Any] | None = Field(None, title="Result")


class ResearchResultResponse(BaseModel):
    """Result payload for the sidecar's /research/{id}/result route."""

    model_config = ConfigDict(extra="forbid")

    report_id: str = Field(..., title="Report Id")
    status: Status
    result: str | None = Field(None, title="Result")
    sources: list[SearchResult] | None = Field(None, title="Sources")
