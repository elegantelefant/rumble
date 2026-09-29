# ABOUTME: Tests for job retention: the purge boundary, what it leaves alone, and the env override.
# ABOUTME: Also proves the purge runs at startup, erases the text from the files, and pidfile ownership.

import json
import os
import sqlite3
from datetime import UTC, datetime, timedelta

import aiosqlite
import pytest_asyncio

from app import _remove_pidfile, _write_pidfile, create_app, lifespan
from services import db

NOW = datetime(2026, 9, 29, 12, 0, 0, tzinfo=UTC)
RETENTION_DAYS = 30
CUTOFF = NOW - timedelta(days=RETENTION_DAYS)


@pytest_asyncio.fixture
async def ephemeral_db(tmp_path):
    await db.init_db(str(tmp_path))
    yield
    await db.close_db()


MARKER = "CONFIDENTIAL-MARKER-7f3a"


def _file_bytes(data_dir) -> bytes:
    """The database file and its WAL, read from disk while the connection is still open."""
    return b"".join(
        (data_dir / name).read_bytes() for name in ("rumble.db", "rumble.db-wal") if (data_dir / name).exists()
    )


async def _job_created_at(created_at: datetime, text: str = "confidential clause") -> str:
    job = await db.create_job("review", json.dumps({"text": text}))
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
    for bad in ("0", "-3", "thirty", "", "800000", str(db.MAX_JOB_RETENTION_DAYS + 1)):
        monkeypatch.setenv(db.JOB_RETENTION_ENV, bad)
        caplog.clear()
        with caplog.at_level("WARNING"):
            assert db.job_retention_days() == db.DEFAULT_JOB_RETENTION_DAYS
        assert any(db.JOB_RETENTION_ENV in r.message for r in caplog.records), bad


def test_retention_accepts_the_maximum(monkeypatch):
    monkeypatch.setenv(db.JOB_RETENTION_ENV, str(db.MAX_JOB_RETENTION_DAYS))
    assert db.job_retention_days() == db.MAX_JOB_RETENTION_DAYS


async def test_purged_text_is_gone_from_the_database_file_and_wal(tmp_path):
    await db.init_db(str(tmp_path))
    try:
        # Long enough to spill into overflow pages as real document text does.
        await _job_created_at(CUTOFF - timedelta(days=1), text=MARKER * 2000)
        assert MARKER.encode() in _file_bytes(tmp_path)

        await db.purge_jobs_older_than(RETENTION_DAYS, now=NOW)

        assert MARKER.encode() not in _file_bytes(tmp_path)
    finally:
        await db.close_db()


async def test_a_database_from_an_earlier_build_is_compacted_once(tmp_path, caplog):
    # What earlier builds left: a row deleted without secure_delete, its text still in the file.
    conn = sqlite3.connect(tmp_path / "rumble.db")
    conn.executescript(db.SCHEMA)
    conn.execute(
        "INSERT INTO jobs (id, type, request, created_at) VALUES ('j', 'review', ?, ?)",
        (MARKER * 2000, NOW.isoformat()),
    )
    conn.commit()
    conn.execute("DELETE FROM jobs")
    conn.commit()
    conn.close()
    assert MARKER.encode() in _file_bytes(tmp_path)

    with caplog.at_level("WARNING"):
        await db.init_db(str(tmp_path))
    try:
        assert MARKER.encode() not in _file_bytes(tmp_path)
        assert any("compacted" in r.message for r in caplog.records)
        version = await (await db._get_db().execute("PRAGMA user_version")).fetchone()
        assert version[0] == db.SCHEMA_VERSION_COMPACTED
    finally:
        await db.close_db()


async def test_compaction_does_not_repeat_on_later_starts(tmp_path, caplog):
    for _ in range(2):
        await db.init_db(str(tmp_path))
        await db.close_db()
    caplog.clear()
    with caplog.at_level("WARNING"):
        await db.init_db(str(tmp_path))
    await db.close_db()
    assert not any("compacted" in r.message for r in caplog.records)


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


async def test_startup_honours_the_retention_override(tmp_path, monkeypatch):
    await db.init_db(str(tmp_path))
    ten_days_old = await _job_created_at(datetime.now(UTC) - timedelta(days=10))
    await db.close_db()
    monkeypatch.setenv(db.JOB_RETENTION_ENV, "7")

    async with lifespan(create_app(data_dir=str(tmp_path), dev=True)):
        pass

    async with aiosqlite.connect(tmp_path / "rumble.db") as conn:
        remaining = [row[0] for row in await (await conn.execute("SELECT id FROM jobs")).fetchall()]
    assert ten_days_old not in remaining


async def test_startup_survives_an_out_of_range_retention_value(tmp_path, monkeypatch):
    monkeypatch.setenv(db.JOB_RETENTION_ENV, "800000")
    async with lifespan(create_app(data_dir=str(tmp_path), dev=True)):
        assert (tmp_path / "sidecar.pid").exists()


def test_a_sidecar_leaves_a_pidfile_it_does_not_own(tmp_path):
    pidfile = tmp_path / "sidecar.pid"
    pidfile.write_text(str(os.getpid() + 1))  # a sibling instance's

    _remove_pidfile(pidfile)

    assert pidfile.read_text() == str(os.getpid() + 1)


def test_a_sidecar_removes_its_own_pidfile(tmp_path):
    pidfile = tmp_path / "sidecar.pid"
    _write_pidfile(pidfile)

    _remove_pidfile(pidfile)

    assert not pidfile.exists()
