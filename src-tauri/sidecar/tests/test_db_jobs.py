# ABOUTME: Unit tests for the job-related database operations.
# ABOUTME: Verifies job CRUD, status transitions, result/error persistence.

import json

import pytest_asyncio

from services import db


@pytest_asyncio.fixture
async def ephemeral_db(tmp_path):
    """Standalone DB fixture for pure service-layer tests."""
    await db.init_db(str(tmp_path))
    yield
    await db.close_db()


# --- Job CRUD ---


async def test_create_job_returns_expected_fields(ephemeral_db):
    job = await db.create_job("draft", json.dumps({"prompt": "NDA"}))
    assert job["id"]
    assert job["type"] == "draft"
    assert job["status"] == "queued"
    assert job["created_at"]


async def test_get_job_found(ephemeral_db):
    created = await db.create_job("review", json.dumps({"text": "contract"}))
    fetched = await db.get_job(created["id"])
    assert fetched is not None
    assert fetched["id"] == created["id"]
    assert fetched["type"] == "review"
    assert fetched["status"] == "queued"
    assert fetched["request"] == json.dumps({"text": "contract"})


async def test_get_job_missing_returns_none(ephemeral_db):
    assert await db.get_job("nonexistent") is None


# --- Status transitions ---


async def test_update_job_status_running_sets_started_at(ephemeral_db):
    job = await db.create_job("draft", "{}")
    await db.update_job_status(job["id"], "running")
    fetched = await db.get_job(job["id"])
    assert fetched["status"] == "running"
    assert fetched["started_at"] is not None


async def test_update_job_status_completed_sets_completed_at(ephemeral_db):
    job = await db.create_job("draft", "{}")
    await db.update_job_status(job["id"], "completed")
    fetched = await db.get_job(job["id"])
    assert fetched["status"] == "completed"
    assert fetched["completed_at"] is not None


async def test_update_job_status_generic(ephemeral_db):
    job = await db.create_job("draft", "{}")
    await db.update_job_status(job["id"], "cancelled")
    fetched = await db.get_job(job["id"])
    assert fetched["status"] == "cancelled"


# --- set_job_result ---


async def test_set_job_result_with_result(ephemeral_db):
    job = await db.create_job("draft", "{}")
    result_json = json.dumps({"draft": "NDA text here"})
    await db.set_job_result(job["id"], result=result_json)
    fetched = await db.get_job(job["id"])
    assert fetched["status"] == "completed"
    assert fetched["result"] == result_json
    assert fetched["error"] is None
    assert fetched["completed_at"] is not None


async def test_set_job_result_with_error(ephemeral_db):
    job = await db.create_job("research", "{}")
    await db.set_job_result(job["id"], error="LLM timeout")
    fetched = await db.get_job(job["id"])
    assert fetched["status"] == "failed"
    assert fetched["error"] == "LLM timeout"
    assert fetched["result"] is None
    assert fetched["completed_at"] is not None


# --- _get_db guard ---


async def test_get_db_before_init_raises():
    """Calling _get_db before init_db should raise RuntimeError."""
    # Save and clear the global connection
    original = db._db
    db._db = None
    try:
        import pytest
        with pytest.raises(RuntimeError, match="database not initialized"):
            db._get_db()
    finally:
        db._db = original
