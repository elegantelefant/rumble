# ABOUTME: Unit tests for the aiosqlite database service.
# ABOUTME: Verifies chat CRUD, message storage, cascade delete, and ordering.

import sqlite3

import pytest
import pytest_asyncio
from services import db
from sqlcipher3 import dbapi2 as sqlcipher


@pytest_asyncio.fixture
async def ephemeral_db(tmp_path, monkeypatch):
    """Standalone DB fixture for pure service-layer tests."""
    monkeypatch.setenv("RUMBLE_DB_KEY", "ab" * 32)
    await db.init_db(str(tmp_path))
    yield
    await db.close_db()


# --- Chat CRUD ---


async def test_create_chat_returns_id(ephemeral_db):
    chat = await db.create_chat(title="Hello")
    assert "id" in chat
    assert chat["title"] == "Hello"
    assert chat["created_at"]


async def test_create_chat_without_title(ephemeral_db):
    chat = await db.create_chat()
    assert chat["title"] is None


async def test_list_chats_ordered_newest_first(ephemeral_db):
    await db.create_chat(title="First")
    await db.create_chat(title="Second")
    chats = await db.list_chats()
    assert len(chats) == 2
    assert chats[0]["title"] == "Second"
    assert chats[1]["title"] == "First"


async def test_get_chat_found(ephemeral_db):
    created = await db.create_chat(title="Test")
    fetched = await db.get_chat(created["id"])
    assert fetched is not None
    assert fetched["id"] == created["id"]
    assert fetched["title"] == "Test"


async def test_get_chat_missing_returns_none(ephemeral_db):
    assert await db.get_chat("nonexistent") is None


async def test_delete_chat(ephemeral_db):
    chat = await db.create_chat(title="Doomed")
    assert await db.delete_chat(chat["id"]) is True
    assert await db.get_chat(chat["id"]) is None


async def test_delete_nonexistent_chat(ephemeral_db):
    assert await db.delete_chat("ghost") is False


async def test_update_chat_title(ephemeral_db):
    chat = await db.create_chat(title="Old")
    updated = await db.update_chat(chat["id"], title="New")
    assert updated["title"] == "New"
    assert updated["updated_at"] is not None


async def test_update_chat_preserves_title_when_none(ephemeral_db):
    chat = await db.create_chat(title="Keep me")
    updated = await db.update_chat(chat["id"], title=None)
    assert updated["title"] == "Keep me"


# --- Message operations ---


async def test_add_and_get_messages(ephemeral_db):
    chat = await db.create_chat()
    await db.add_message(chat["id"], "user", "Hello")
    await db.add_message(chat["id"], "assistant", "Hi there")
    messages = await db.get_messages(chat["id"])
    assert len(messages) == 2
    assert messages[0]["role"] == "user"
    assert messages[0]["content"] == "Hello"
    assert messages[1]["role"] == "assistant"


async def test_messages_ordered_by_created_at(ephemeral_db):
    chat = await db.create_chat()
    await db.add_message(chat["id"], "user", "First")
    await db.add_message(chat["id"], "assistant", "Second")
    await db.add_message(chat["id"], "user", "Third")
    messages = await db.get_messages(chat["id"])
    contents = [m["content"] for m in messages]
    assert contents == ["First", "Second", "Third"]


async def test_add_message_updates_chat_timestamp(ephemeral_db):
    chat = await db.create_chat()
    assert (await db.get_chat(chat["id"]))["updated_at"] is None
    await db.add_message(chat["id"], "user", "Trigger update")
    assert (await db.get_chat(chat["id"]))["updated_at"] is not None


async def test_delete_chat_cascades_to_messages(ephemeral_db):
    chat = await db.create_chat()
    await db.add_message(chat["id"], "user", "Gone soon")
    await db.delete_chat(chat["id"])
    messages = await db.get_messages(chat["id"])
    assert messages == []


async def test_get_messages_for_empty_chat(ephemeral_db):
    chat = await db.create_chat()
    assert await db.get_messages(chat["id"]) == []


async def test_message_fields(ephemeral_db):
    chat = await db.create_chat()
    msg = await db.add_message(chat["id"], "user", "Test content")
    assert msg["id"]
    assert msg["chat_id"] == chat["id"]
    assert msg["role"] == "user"
    assert msg["content"] == "Test content"
    assert msg["created_at"]


async def test_database_file_is_unreadable_without_the_key(ephemeral_db, tmp_path):
    chat = await db.create_chat("Encrypted matter")
    await db.add_message(chat["id"], "user", "Confidential document")
    await db.close_db()
    with (
        sqlite3.connect(tmp_path / "rumble.db") as connection,
        pytest.raises(sqlite3.DatabaseError, match="file is not a database"),
    ):
        connection.execute("SELECT * FROM chats").fetchall()

    connection = sqlcipher.connect(tmp_path / "rumble.db")
    try:
        connection.execute(f"PRAGMA key = \"x'{ 'ab' * 32 }'\"")
        assert connection.execute("SELECT title FROM chats").fetchall() == [("Encrypted matter",)]
        assert connection.execute("SELECT content FROM messages").fetchall() == [("Confidential document",)]
    finally:
        connection.close()
    await db.init_db(str(tmp_path))
    assert (await db.get_chat(chat["id"]))["title"] == "Encrypted matter"


@pytest.mark.parametrize("key", [None, "", "not-hex", "ab" * 31, "ab" * 32 + "'\";"])
async def test_missing_or_invalid_key_refuses_init(tmp_path, monkeypatch, key):
    if key is None:
        monkeypatch.delenv("RUMBLE_DB_KEY", raising=False)
    else:
        monkeypatch.setenv("RUMBLE_DB_KEY", key)
    try:
        with pytest.raises(RuntimeError, match="database key"):
            await db.init_db(str(tmp_path))
        assert not (tmp_path / "rumble.db").exists()
        with pytest.raises(RuntimeError, match="not initialized"):
            db._get_db()
    finally:
        await db.close_db()


async def test_plaintext_database_migrates_every_row(tmp_path, monkeypatch):
    path = tmp_path / "rumble.db"
    with sqlite3.connect(path) as connection:
        connection.executescript(db.SCHEMA)
        connection.execute("PRAGMA user_version=1")
        for i in range(3):
            connection.execute("INSERT INTO chats VALUES (?, ?, ?, ?)", (str(i), f"Matter {i}", "now", None))
            connection.execute("INSERT INTO messages VALUES (?, ?, ?, ?, ?)", (str(i), str(i), "user", f"Document {i}", "now"))
            connection.execute("INSERT INTO jobs (id, type, request, created_at) VALUES (?, ?, ?, ?)", (str(i), "review", f"Request {i}", "now"))
        connection.commit()
        expected = {table: connection.execute(f"SELECT * FROM {table}").fetchall() for table in ("chats", "messages", "jobs")}
    monkeypatch.setenv("RUMBLE_DB_KEY", "ab" * 32)
    try:
        await db.init_db(str(tmp_path))
        for table, rows in expected.items():
            cursor = await db._get_db().execute(f"SELECT * FROM {table}")
            assert [tuple(row) for row in await cursor.fetchall()] == rows
        assert (await (await db._get_db().execute("PRAGMA user_version")).fetchone())[0] == 1
    finally:
        await db.close_db()
    assert {p.name for p in tmp_path.iterdir()} == {"rumble.db"}
    with (
        sqlite3.connect(path) as connection,
        pytest.raises(sqlite3.DatabaseError, match="file is not a database"),
    ):
        connection.execute("SELECT * FROM chats")


async def test_wrong_key_preserves_encrypted_database(ephemeral_db, tmp_path, monkeypatch):
    chat = await db.create_chat("Keep this")
    await db.close_db()
    original = (tmp_path / "rumble.db").read_bytes()
    monkeypatch.setenv("RUMBLE_DB_KEY", "cd" * 32)
    with pytest.raises(sqlcipher.DatabaseError, match="file is not a database"):
        await db.init_db(str(tmp_path))
    assert db._db is None
    assert (tmp_path / "rumble.db").read_bytes() == original
    monkeypatch.setenv("RUMBLE_DB_KEY", "ab" * 32)
    await db.init_db(str(tmp_path))
    assert await db.get_chat(chat["id"]) is not None


@pytest.mark.parametrize("interrupted_after_backup", [False, True])
async def test_interrupted_migration_recovers_and_erases_plaintext(tmp_path, monkeypatch, interrupted_after_backup):
    path = tmp_path / "rumble.db"
    backup = path.with_suffix(".db.plaintext")
    with sqlite3.connect(path) as connection:
        connection.executescript(db.SCHEMA)
        connection.execute("INSERT INTO chats VALUES ('kept', 'Confidential', 'now', NULL)")
        connection.commit()
    original_size = path.stat().st_size
    real_replace = type(path).replace
    real_unlink = type(path).unlink
    erased = []

    def interrupt_replace(source, target):
        if source == (path.with_suffix(".db.encrypted") if interrupted_after_backup else path):
            raise OSError("simulated interruption")
        return real_replace(source, target)

    def observe_unlink(source, *args, **kwargs):
        if source == backup:
            erased.append(source.read_bytes())
        return real_unlink(source, *args, **kwargs)

    monkeypatch.setenv("RUMBLE_DB_KEY", "ab" * 32)
    monkeypatch.setattr(type(path), "replace", interrupt_replace)
    with pytest.raises(OSError, match="simulated interruption"):
        await db.init_db(str(tmp_path))
    assert db._db is None
    monkeypatch.setattr(type(path), "replace", real_replace)
    monkeypatch.setattr(type(path), "unlink", observe_unlink)
    try:
        await db.init_db(str(tmp_path))
        assert (await db.get_chat("kept"))["title"] == "Confidential"
    finally:
        await db.close_db()
    assert erased == [bytes(original_size)]
    assert {p.name for p in tmp_path.iterdir()} == {"rumble.db"}
