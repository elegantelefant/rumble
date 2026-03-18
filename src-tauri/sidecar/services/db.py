# ABOUTME: Async SQLite database layer for chat persistence.
# ABOUTME: Manages schema creation, chat CRUD, and message storage via aiosqlite.

import uuid
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import aiosqlite

_db: aiosqlite.Connection | None = None

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
    status TEXT NOT NULL DEFAULT 'queued',
    request TEXT NOT NULL,
    result TEXT,
    error TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
);
"""


def _now() -> str:
    return datetime.now(UTC).isoformat()


def _uuid() -> str:
    return str(uuid.uuid4())


async def init_db(data_dir: str | None = None) -> None:
    global _db
    if data_dir:
        path = Path(data_dir) / "rumble.db"
        path.parent.mkdir(parents=True, exist_ok=True)
    else:
        path = Path("rumble.db")
    _db = await aiosqlite.connect(str(path))
    _db.row_factory = aiosqlite.Row
    await _db.execute("PRAGMA journal_mode=WAL")
    await _db.execute("PRAGMA foreign_keys=ON")
    await _db.executescript(SCHEMA)
    await _db.commit()


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
    await db.execute("BEGIN")
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


async def update_job_status(job_id: str, status: str) -> None:
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
