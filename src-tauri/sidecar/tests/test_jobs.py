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
