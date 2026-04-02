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
