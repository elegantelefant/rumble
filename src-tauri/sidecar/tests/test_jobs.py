# ABOUTME: Tests for async job endpoints — draft, review, research.
# ABOUTME: Validates job creation, polling, completion, and error handling.

import asyncio
import json


# --- Draft ---

async def test_draft_creates_job(client):
    resp = await client.post("/draft", json={"prompt": "NDA between two parties"})
    assert resp.status_code == 200
    data = resp.json()
    assert "job_id" in data
    assert data["status"] == "queued"
    assert "/draft/" in data["poll_url"]
    assert "/result" in data["poll_url"]


async def test_draft_completes(client):
    resp = await client.post("/draft", json={"prompt": "NDA between two parties"})
    job_id = resp.json()["job_id"]
    # Let the background task complete
    await asyncio.sleep(0.1)
    result = await client.get(f"/draft/{job_id}/result")
    assert result.status_code == 200
    data = result.json()
    assert data["status"] == "completed"
    assert data["result"] is not None
    assert "draft" in data["result"] or "Draft" in json.dumps(data["result"])


async def test_draft_nonexistent_job(client):
    resp = await client.get("/draft/no-such-id/result")
    assert resp.status_code == 404


async def test_draft_retry_logs_the_missing_fields_key_not_its_value(client, caplog):
    """A retry's log line names which field was missing by key, never by
    value (#46) -- a value is the user's own document content."""
    from unittest.mock import AsyncMock, patch

    import routes.jobs as jobs_module

    first_attempt = "A draft that never mentions the employee by name."
    second_attempt = "A complete draft naming Tester as the employee."

    with (
        patch("services.llm.run_single_turn", new=AsyncMock(side_effect=[first_attempt, second_attempt])),
        caplog.at_level("WARNING", logger=jobs_module.__name__),
    ):
        resp = await client.post("/draft", json={
            "prompt": "Draft an agreement",
            "fields": [{"key": "employeeName", "label": "Employee Name", "value": "Tester"}],
        })
        job_id = resp.json()["job_id"]
        await asyncio.sleep(0.1)
        result = await client.get(f"/draft/{job_id}/result")

    assert result.json()["result"]["draft"] == second_attempt
    retry_logs = [r.message for r in caplog.records if "Draft retry" in r.message]
    assert len(retry_logs) == 1
    assert "employeeName" in retry_logs[0]
    assert "Tester" not in retry_logs[0]


async def test_draft_strips_markdown_from_the_final_draft(client):
    """The model sometimes wraps clause headings in markdown; the /draft
    route's own response must come back with it stripped (#46), not just
    the substitution helper in isolation."""
    from unittest.mock import AsyncMock, patch

    raw = "# SERVICE AGREEMENT\n\n**1. TERM**\nThis agreement begins on the Start Date."

    with patch("services.llm.run_single_turn", new=AsyncMock(return_value=raw)):
        resp = await client.post("/draft", json={"prompt": "Draft an agreement"})
        job_id = resp.json()["job_id"]
        await asyncio.sleep(0.1)
        result = await client.get(f"/draft/{job_id}/result")

    draft = result.json()["result"]["draft"]
    assert draft == "SERVICE AGREEMENT\n\n1. TERM\nThis agreement begins on the Start Date."


# --- Review ---

async def test_review_creates_job(client):
    resp = await client.post("/review", json={"text": "This agreement shall be governed by..."})
    assert resp.status_code == 200
    data = resp.json()
    assert "job_id" in data
    assert data["status"] == "queued"


async def test_review_completes(client):
    resp = await client.post("/review", json={
        "text": "This agreement shall be governed by the laws of Singapore.",
        "instructions": "Check for ambiguities",
    })
    job_id = resp.json()["job_id"]
    await asyncio.sleep(0.1)
    result = await client.get(f"/review/{job_id}/result")
    assert result.status_code == 200
    data = result.json()
    assert data["status"] == "completed"
    assert data["result"] is not None


# --- Research ---

async def test_research_creates_job(client):
    resp = await client.post("/research", json={"question": "What are the elements of negligence?"})
    assert resp.status_code == 200
    data = resp.json()
    assert "report_id" in data
    assert data["status"] == "queued"


async def test_research_status(client):
    resp = await client.post("/research", json={"question": "What are the elements of negligence?"})
    report_id = resp.json()["report_id"]
    await asyncio.sleep(0.1)
    status = await client.get(f"/research/{report_id}")
    assert status.status_code == 200
    data = status.json()
    assert data["id"] == report_id
    assert data["status"] in ("queued", "running", "completed")


async def test_research_result(client):
    resp = await client.post("/research", json={"question": "What are the elements of negligence?"})
    report_id = resp.json()["report_id"]
    await asyncio.sleep(0.1)
    result = await client.get(f"/research/{report_id}/result")
    assert result.status_code == 200
    data = result.json()
    assert data["report_id"] == report_id


async def test_research_nonexistent_job(client):
    resp = await client.get("/research/no-such-id")
    assert resp.status_code == 404


async def test_job_task_tracked_in_tasks_dict(client):
    """Background tasks are stored in _tasks dict for lifecycle tracking."""
    from services.jobs import _tasks

    resp = await client.post("/draft", json={"prompt": "NDA"})
    job_id = resp.json()["job_id"]
    # Task should be tracked (may already be done, but was tracked)
    await asyncio.sleep(0.2)
    # After completion, done-callback should have cleaned up
    assert job_id not in _tasks


async def test_research_result_preserves_sources(client):
    """Research sources from LLM response should appear in the result."""
    resp = await client.post("/research", json={"question": "What are torts?"})
    report_id = resp.json()["report_id"]
    await asyncio.sleep(0.2)
    result = await client.get(f"/research/{report_id}/result")
    data = result.json()
    assert "sources" in data


# --- Failed jobs ---

async def test_failed_draft_returns_error_in_result(client):
    """A draft job that fails should have status=failed and error in result."""
    from unittest.mock import AsyncMock, patch

    async def _boom(*args, **kwargs):
        raise RuntimeError("Ollama unreachable")

    with patch("services.llm.run_single_turn", new=AsyncMock(side_effect=_boom)):
        resp = await client.post("/draft", json={"prompt": "NDA"})
        job_id = resp.json()["job_id"]
        await asyncio.sleep(0.2)
        result = await client.get(f"/draft/{job_id}/result")

    assert result.status_code == 200
    data = result.json()
    assert data["status"] == "failed"
    assert data["result"] is not None
    assert "Ollama unreachable" in data["result"]["error"]


async def test_failed_research_returns_error_in_result(client):
    """A research job that fails should have status=failed and error in result."""
    from unittest.mock import AsyncMock, patch

    async def _boom(*args, **kwargs):
        raise RuntimeError("model not found")

    with patch("services.llm.run_single_turn", new=AsyncMock(side_effect=_boom)):
        resp = await client.post("/research", json={"question": "What is tort?"})
        report_id = resp.json()["report_id"]
        await asyncio.sleep(0.2)
        result = await client.get(f"/research/{report_id}/result")

    assert result.status_code == 200
    data = result.json()
    assert data["status"] == "failed"
    assert data["result"] is not None
    assert "model not found" in data["result"]


async def test_failed_review_returns_error_in_result(client):
    """A review job that fails should have status=failed and error in result."""
    from unittest.mock import AsyncMock, patch

    async def _boom(*args, **kwargs):
        raise ValueError("LLM returned unparseable output")

    with patch("services.llm.run_single_turn", new=AsyncMock(side_effect=_boom)):
        resp = await client.post("/review", json={"text": "Some contract text."})
        job_id = resp.json()["job_id"]
        await asyncio.sleep(0.2)
        result = await client.get(f"/review/{job_id}/result")

    assert result.status_code == 200
    data = result.json()
    assert data["status"] == "failed"
    assert data["result"] is not None
    assert "unparseable" in data["result"]["error"]


async def test_failed_research_status_includes_error(client):
    """The research status endpoint should include the error field for failed jobs."""
    from unittest.mock import AsyncMock, patch

    async def _boom(*args, **kwargs):
        raise RuntimeError("connection refused")

    with patch("services.llm.run_single_turn", new=AsyncMock(side_effect=_boom)):
        resp = await client.post("/research", json={"question": "What is negligence?"})
        report_id = resp.json()["report_id"]
        await asyncio.sleep(0.2)
        status = await client.get(f"/research/{report_id}")

    assert status.status_code == 200
    data = status.json()
    assert data["status"] == "failed"
    assert "connection refused" in data["error"]
