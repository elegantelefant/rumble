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
"""


def _now() -> str:
    return datetime.now(UTC).isoformat()


def _uuid() -> str:
    return str(uuid.uuid4())


async def init_db(data_dir: str | None = None) -> None:
    global _db
    if data_dir:
        path = Path(data_dir) / "ivory.db"
        path.parent.mkdir(parents=True, exist_ok=True)
    else:
        path = Path("ivory.db")
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
    await db.execute(
        "INSERT INTO messages (id, chat_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
        (msg_id, chat_id, role, content, now),
    )
    await db.execute(
        "UPDATE chats SET updated_at = ? WHERE id = ?", (now, chat_id)
    )
    await db.commit()
    return {"id": msg_id, "chat_id": chat_id, "role": role, "content": content, "created_at": now}


async def get_messages(chat_id: str) -> list[dict[str, Any]]:
    db = _get_db()
    cursor = await db.execute(
        "SELECT id, chat_id, role, content, created_at FROM messages WHERE chat_id = ? ORDER BY created_at",
        (chat_id,),
    )
    rows = await cursor.fetchall()
    return [dict(row) for row in rows]
