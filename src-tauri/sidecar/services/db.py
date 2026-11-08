# ABOUTME: Async SQLite database layer for chat persistence.
# ABOUTME: Manages schema creation, chat CRUD, message and job storage, and job retention via aiosqlite.

import logging
import os
import re
import uuid
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any, Literal

import aiosqlite
from sqlcipher3 import dbapi2 as sqlite3

logger = logging.getLogger(__name__)

_db: aiosqlite.Connection | None = None

# Jobs hold full document text in request/result, so they are not kept forever (#57).
DEFAULT_JOB_RETENTION_DAYS = 30
# A century: far beyond any real policy, and well inside what datetime arithmetic can represent.
MAX_JOB_RETENTION_DAYS = 36500
JOB_RETENTION_ENV = "RUMBLE_JOB_RETENTION_DAYS"

# PRAGMA user_version once the database has been compacted to erase what earlier
# builds deleted without secure_delete.
SCHEMA_VERSION_COMPACTED = 1

SCHEMA = """
CREATE TABLE IF NOT EXISTS chats (
    id TEXT PRIMARY KEY,
    title TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages(chat_id);

CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued', 'running', 'completed', 'failed')),
    request TEXT NOT NULL,
    result TEXT,
    error TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
"""

JobStatus = Literal["queued", "running", "completed", "failed"]


def _now() -> str:
    return datetime.now(UTC).isoformat()


def _uuid() -> str:
    return str(uuid.uuid4())


def _secure_remove(path: Path) -> None:
    """Overwrite and unlink the old plaintext inode (SSD/snapshot copies are outside our control)."""
    if not path.exists():
        return
    with path.open("r+b") as old:
        remaining = os.fstat(old.fileno()).st_size
        zeros = bytes(1024 * 1024)
        while remaining:
            written = old.write(zeros[:min(remaining, len(zeros))])
            remaining -= written
        old.flush()
        os.fsync(old.fileno())
    path.unlink()


def _keyed_connection(path: Path, key: str):
    connection = sqlite3.connect(str(path))
    try:
        # First SQL statement; key is validated hex, never passed through aiosqlite's SQL logger.
        connection.execute(f"PRAGMA key = \"x'{key}'\"")
        if not connection.execute("PRAGMA cipher_version").fetchone():
            raise RuntimeError("SQLCipher is unavailable")
        connection.execute("SELECT count(*) FROM sqlite_master").fetchone()
        return connection
    except BaseException:
        connection.close()
        raise


def _migrate_plaintext(path: Path, key: str, backup: Path) -> None:
    encrypted = path.with_name(path.name + ".migrating-sqlcipher")
    encrypted.unlink(missing_ok=True)  # An interrupted export; original is still intact.
    source = sqlite3.connect(str(path))
    try:
        try:
            # Fold committed WAL frames into the source, then remove plaintext journals on close.
            source.execute("PRAGMA wal_checkpoint(TRUNCATE)")
            source.execute("PRAGMA journal_mode=DELETE")
            version = source.execute("PRAGMA user_version").fetchone()[0]
            source.execute("ATTACH DATABASE ? AS encrypted KEY ?", (str(encrypted), f"x'{key}'"))
            source.execute("SELECT sqlcipher_export('encrypted')")
            source.execute(f"PRAGMA encrypted.user_version = {version}")
            source.commit()
            source.execute("DETACH DATABASE encrypted")
        finally:
            source.close()
        check = _keyed_connection(encrypted, key)
        check.close()
        with encrypted.open("rb") as ready:
            os.fsync(ready.fileno())
    except BaseException:
        # Close attached databases before unlinking, including on Windows.
        encrypted.unlink(missing_ok=True)
        raise
    # Keep the complete source until the encrypted copy occupies the database path.
    os.replace(path, backup)
    try:
        os.replace(encrypted, path)
    except BaseException:
        os.replace(backup, path)
        raise
    _secure_remove(backup)


def _open_database(path: Path, key: str):
    backup = path.with_name(path.name + ".migrating-sqlcipher-plaintext")
    if backup.exists() and not path.exists():
        os.replace(backup, path)  # Interrupted between the two renames; retry export.
    if path.exists():
        with path.open("rb") as existing:
            plaintext = existing.read(16) == b"SQLite format 3\x00"
        if plaintext:
            _migrate_plaintext(path, key, backup)
    connection = _keyed_connection(path, key)
    try:
        # A crash after installation may have left the source awaiting erasure.
        _secure_remove(backup)
    except BaseException:
        connection.close()
        raise
    return connection


async def init_db(data_dir: str | None = None) -> None:
    global _db
    key = os.environ.get("RUMBLE_DB_KEY", "")
    if not re.fullmatch(r"[0-9a-fA-F]{64}", key):
        raise RuntimeError("RUMBLE_DB_KEY must contain a 32-byte hexadecimal key")
    path = Path(data_dir or ".") / "rumble.db"
    path.parent.mkdir(parents=True, exist_ok=True)
    existed = path.exists()
    connection = await aiosqlite.Connection(lambda: _open_database(path, key), iter_chunk_size=64)
    _db = connection
    try:
        connection.row_factory = sqlite3.Row
        await connection.execute("PRAGMA journal_mode=WAL")
        await connection.execute("PRAGMA foreign_keys=ON")
        await connection.execute("PRAGMA secure_delete=ON")
        await connection.executescript(SCHEMA)
        await connection.commit()
        await _compact_once(existed, path)
    except BaseException:
        await close_db()
        raise


async def _compact_once(existed: bool, path: Path) -> None:
    """VACUUM a database from before secure_delete, once: its free pages still hold deleted content."""
    cursor = await _db.execute("PRAGMA user_version")
    if (await cursor.fetchone())[0] >= SCHEMA_VERSION_COMPACTED:
        return
    if existed:
        try:
            await _db.execute("VACUUM")
            await _checkpoint()
        except sqlite3.Error as exc:
            # Best effort: a full disk or a lock must not stop the sidecar starting. user_version
            # stays unset, so the next start tries again.
            logger.error("could not compact %s (will retry next start): %s", path, exc)
            return
        logger.warning("compacted %s once to erase content deleted by earlier builds", path)
    await _db.execute(f"PRAGMA user_version = {SCHEMA_VERSION_COMPACTED}")
    await _db.commit()


async def _checkpoint() -> None:
    """Copy the WAL into the database file and truncate it, so superseded pages leave the WAL too."""
    await _get_db().execute("PRAGMA wal_checkpoint(TRUNCATE)")


async def close_db() -> None:
    global _db
    if _db:
        await _db.close()
        _db = None


def _get_db() -> aiosqlite.Connection:
    if _db is None:
        raise RuntimeError("database not initialized")
    return _db


# --- Chat operations ---

async def create_chat(title: str | None = None) -> dict[str, Any]:
    db = _get_db()
    chat_id = _uuid()
    now = _now()
    await db.execute(
        "INSERT INTO chats (id, title, created_at) VALUES (?, ?, ?)",
        (chat_id, title, now),
    )
    await db.commit()
    return {"id": chat_id, "title": title, "created_at": now}


async def list_chats() -> list[dict[str, Any]]:
    db = _get_db()
    cursor = await db.execute(
        "SELECT id, title, created_at, updated_at FROM chats ORDER BY created_at DESC"
    )
    rows = await cursor.fetchall()
    return [dict(row) for row in rows]


async def get_chat(chat_id: str) -> dict[str, Any] | None:
    db = _get_db()
    cursor = await db.execute(
        "SELECT id, title, created_at, updated_at FROM chats WHERE id = ?",
        (chat_id,),
    )
    row = await cursor.fetchone()
    return dict(row) if row else None


async def delete_chat(chat_id: str) -> bool:
    db = _get_db()
    cursor = await db.execute("DELETE FROM chats WHERE id = ?", (chat_id,))
    await db.commit()
    return cursor.rowcount > 0


async def update_chat(chat_id: str, title: str | None = None) -> dict[str, Any] | None:
    db = _get_db()
    now = _now()
    await db.execute(
        "UPDATE chats SET title = COALESCE(?, title), updated_at = ? WHERE id = ?",
        (title, now, chat_id),
    )
    await db.commit()
    return await get_chat(chat_id)


# --- Message operations ---

async def add_message(chat_id: str, role: str, content: str) -> dict[str, Any]:
    db = _get_db()
    msg_id = _uuid()
    now = _now()
    try:
        await db.execute(
            "INSERT INTO messages (id, chat_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
            (msg_id, chat_id, role, content, now),
        )
        await db.execute(
            "UPDATE chats SET updated_at = ? WHERE id = ?", (now, chat_id)
        )
        await db.commit()
    except Exception:
        await db.rollback()
        raise
    return {"id": msg_id, "chat_id": chat_id, "role": role, "content": content, "created_at": now}


async def delete_message(msg_id: str) -> None:
    db = _get_db()
    await db.execute("DELETE FROM messages WHERE id = ?", (msg_id,))
    await db.commit()


async def get_messages(chat_id: str) -> list[dict[str, Any]]:
    db = _get_db()
    cursor = await db.execute(
        "SELECT id, chat_id, role, content, created_at FROM messages WHERE chat_id = ? ORDER BY created_at",
        (chat_id,),
    )
    rows = await cursor.fetchall()
    return [dict(row) for row in rows]


# --- Job operations ---

async def create_job(job_type: str, request_data: str) -> dict[str, Any]:
    db = _get_db()
    job_id = _uuid()
    now = _now()
    await db.execute(
        "INSERT INTO jobs (id, type, status, request, created_at) VALUES (?, ?, 'queued', ?, ?)",
        (job_id, job_type, request_data, now),
    )
    await db.commit()
    return {"id": job_id, "type": job_type, "status": "queued", "created_at": now}


async def get_job(job_id: str) -> dict[str, Any] | None:
    db = _get_db()
    cursor = await db.execute(
        "SELECT id, type, status, request, result, error, created_at, started_at, completed_at FROM jobs WHERE id = ?",
        (job_id,),
    )
    row = await cursor.fetchone()
    return dict(row) if row else None


async def update_job_status(job_id: str, status: JobStatus) -> None:
    db = _get_db()
    now = _now()
    if status == "running":
        await db.execute("UPDATE jobs SET status = ?, started_at = ? WHERE id = ?", (status, now, job_id))
    elif status in ("completed", "failed"):
        await db.execute("UPDATE jobs SET status = ?, completed_at = ? WHERE id = ?", (status, now, job_id))
    else:
        await db.execute("UPDATE jobs SET status = ? WHERE id = ?", (status, job_id))
    await db.commit()


async def set_job_result(job_id: str, result: str | None = None, error: str | None = None) -> None:
    db = _get_db()
    now = _now()
    status = "failed" if error is not None else ("completed" if result is not None else "failed")
    await db.execute(
        "UPDATE jobs SET status = ?, result = ?, error = ?, completed_at = ? WHERE id = ?",
        (status, result, error, now, job_id),
    )
    await db.commit()


# --- Retention ---

def job_retention_days() -> int:
    """RUMBLE_JOB_RETENTION_DAYS as a whole number of days (1 to MAX_JOB_RETENTION_DAYS), else the default."""
    raw = os.environ.get(JOB_RETENTION_ENV)
    if raw is None:
        return DEFAULT_JOB_RETENTION_DAYS
    try:
        days = int(raw)
    except ValueError:
        days = 0
    if not 1 <= days <= MAX_JOB_RETENTION_DAYS:
        logger.warning(
            "Ignoring %s=%r (need a whole number of days from 1 to %d); keeping jobs %d days",
            JOB_RETENTION_ENV,
            raw,
            MAX_JOB_RETENTION_DAYS,
            DEFAULT_JOB_RETENTION_DAYS,
        )
        return DEFAULT_JOB_RETENTION_DAYS
    return days


async def purge_jobs_older_than(days: int, now: datetime | None = None) -> int:
    """Delete jobs created more than `days` before `now`; return how many. Chats and messages are kept."""
    cutoff = ((now or datetime.now(UTC)) - timedelta(days=days)).isoformat()
    db = _get_db()
    # _now()'s UTC isoformat() strings sort chronologically: fixed width through the seconds, and
    # where isoformat() omits zero microseconds, "+" sorts before ".".
    cursor = await db.execute("DELETE FROM jobs WHERE created_at < ?", (cutoff,))
    await db.commit()
    if cursor.rowcount:
        # secure_delete zeroes the rows in the pages it writes to the WAL; the database file and
        # the WAL's older frames keep the old text until a checkpoint replaces and truncates them.
        await _checkpoint()
    return cursor.rowcount
