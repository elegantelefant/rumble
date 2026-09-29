# ABOUTME: Tests for job retention: the purge boundary, what it leaves alone, and the env override.
# ABOUTME: Also proves the purge runs at sidecar startup through the real lifespan.

import json
from datetime import UTC, datetime, timedelta

import aiosqlite
import pytest_asyncio

from app import create_app, lifespan
from services import db

NOW = datetime(2026, 9, 29, 12, 0, 0, tzinfo=UTC)
RETENTION_DAYS = 30
CUTOFF = NOW - timedelta(days=RETENTION_DAYS)


@pytest_asyncio.fixture
async def ephemeral_db(tmp_path):
    await db.init_db(str(tmp_path))
    yield
    await db.close_db()


async def _job_created_at(created_at: datetime) -> str:
    job = await db.create_job("review", json.dumps({"text": "confidential clause"}))
    await db._get_db().execute("UPDATE jobs SET created_at = ? WHERE id = ?", (created_at.isoformat(), job["id"]))
    await db._get_db().commit()
    return job["id"]


async def test_purge_deletes_a_job_one_second_older_than_the_cutoff(ephemeral_db):
    job_id = await _job_created_at(CUTOFF - timedelta(seconds=1))
    assert await db.purge_jobs_older_than(RETENTION_DAYS, now=NOW) == 1
    assert await db.get_job(job_id) is None


async def test_purge_keeps_a_job_created_exactly_at_the_cutoff(ephemeral_db):
    job_id = await _job_created_at(CUTOFF)
    assert await db.purge_jobs_older_than(RETENTION_DAYS, now=NOW) == 0
    assert await db.get_job(job_id) is not None


async def test_purge_keeps_a_recent_job(ephemeral_db):
    job_id = await _job_created_at(NOW - timedelta(days=1))
    await db.purge_jobs_older_than(RETENTION_DAYS, now=NOW)
    assert await db.get_job(job_id) is not None


async def test_purge_orders_a_whole_second_timestamp_before_a_fractional_cutoff(ephemeral_db):
    # isoformat() drops ".000000" at a whole second; that row is still half a second older than the cutoff.
    job_id = await _job_created_at(CUTOFF)
    assert await db.purge_jobs_older_than(RETENTION_DAYS, now=NOW + timedelta(milliseconds=500)) == 1
    assert await db.get_job(job_id) is None


async def test_purge_orders_a_fractional_timestamp_after_a_whole_second_cutoff(ephemeral_db):
    job_id = await _job_created_at(CUTOFF + timedelta(milliseconds=500))
    assert await db.purge_jobs_older_than(RETENTION_DAYS, now=NOW) == 0
    assert await db.get_job(job_id) is not None


async def test_purge_leaves_old_chats_and_messages_alone(ephemeral_db):
    chat = await db.create_chat("Old matter")
    await db.add_message(chat["id"], "user", "hello")
    await db._get_db().execute("UPDATE chats SET created_at = ?", ((CUTOFF - timedelta(days=365)).isoformat(),))
    await db._get_db().commit()

    await db.purge_jobs_older_than(RETENTION_DAYS, now=NOW)

    assert await db.get_chat(chat["id"]) is not None
    assert len(await db.get_messages(chat["id"])) == 1


def test_retention_defaults_to_thirty_days(monkeypatch):
    monkeypatch.delenv(db.JOB_RETENTION_ENV, raising=False)
    assert db.job_retention_days() == db.DEFAULT_JOB_RETENTION_DAYS == 30


def test_retention_env_override_is_honoured(monkeypatch):
    monkeypatch.setenv(db.JOB_RETENTION_ENV, "7")
    assert db.job_retention_days() == 7


def test_retention_bad_env_value_falls_back_with_a_warning(monkeypatch, caplog):
    for bad in ("0", "-3", "thirty", ""):
        monkeypatch.setenv(db.JOB_RETENTION_ENV, bad)
        caplog.clear()
        with caplog.at_level("WARNING"):
            assert db.job_retention_days() == db.DEFAULT_JOB_RETENTION_DAYS
        assert any(db.JOB_RETENTION_ENV in r.message for r in caplog.records), bad


async def test_secure_delete_is_on(ephemeral_db):
    cursor = await db._get_db().execute("PRAGMA secure_delete")
    assert (await cursor.fetchone())[0] == 1


async def test_startup_purges_expired_jobs(tmp_path, monkeypatch, caplog):
    monkeypatch.delenv(db.JOB_RETENTION_ENV, raising=False)
    await db.init_db(str(tmp_path))
    expired = await _job_created_at(datetime.now(UTC) - timedelta(days=RETENTION_DAYS + 1))
    fresh = await _job_created_at(datetime.now(UTC))
    await db.close_db()

    application = create_app(data_dir=str(tmp_path), dev=True)
    with caplog.at_level("WARNING"):
        async with lifespan(application):
            pass

    async with aiosqlite.connect(tmp_path / "rumble.db") as conn:
        remaining = {row[0] for row in await (await conn.execute("SELECT id FROM jobs")).fetchall()}
    assert remaining == {fresh}
    assert expired not in remaining
    assert any("deleted 1 job(s) older than 30 days" in r.message for r in caplog.records)
