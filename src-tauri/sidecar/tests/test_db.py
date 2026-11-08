# ABOUTME: Unit tests for the aiosqlite database service.
# ABOUTME: Verifies chat CRUD, message storage, cascade delete, and ordering.

import sqlite3
from contextlib import closing

import pytest
import pytest_asyncio
from services import db
from sqlcipher3 import dbapi2 as cipher


@pytest_asyncio.fixture
async def ephemeral_db(tmp_path):
    """Standalone DB fixture for pure service-layer tests."""
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
    await db.create_chat("confidential chat")
    await db.close_db()
    with (
        closing(sqlite3.connect(tmp_path / "rumble.db")) as plain,
        pytest.raises(sqlite3.DatabaseError, match="file is not a database"),
    ):
        plain.execute("SELECT * FROM chats").fetchall()
    with closing(cipher.connect(tmp_path / "rumble.db")) as unlocked:
        unlocked.execute("PRAGMA key = \"x'" + "ab" * 32 + "'\"")
        assert unlocked.execute("SELECT title FROM chats").fetchall() == [("confidential chat",)]
    await db.init_db(str(tmp_path))
    assert (await db.list_chats())[0]["title"] == "confidential chat"


@pytest.mark.parametrize("key", [None, "", "wrong", "g" * 64])
async def test_missing_or_invalid_key_refuses_to_initialize(tmp_path, monkeypatch, key):
    if key is None:
        monkeypatch.delenv("RUMBLE_DB_KEY")
    else:
        monkeypatch.setenv("RUMBLE_DB_KEY", key)
    try:
        with pytest.raises(RuntimeError, match="RUMBLE_DB_KEY"):
            await db.init_db(str(tmp_path))
        assert not (tmp_path / "rumble.db").exists()
        with pytest.raises(RuntimeError, match="not initialized"):
            db._get_db()
    finally:
        await db.close_db()


async def test_wrong_key_refuses_existing_database(ephemeral_db, tmp_path, monkeypatch):
    await db.create_chat("keep this")
    await db.close_db()
    before = (tmp_path / "rumble.db").read_bytes()
    monkeypatch.setenv("RUMBLE_DB_KEY", "cd" * 32)
    try:
        with pytest.raises(cipher.DatabaseError, match="file is not a database"):
            await db.init_db(str(tmp_path))
        assert (tmp_path / "rumble.db").read_bytes() == before
        with pytest.raises(RuntimeError, match="not initialized"):
            db._get_db()
    finally:
        await db.close_db()


async def test_plaintext_fixture_migrates_every_row(tmp_path):
    path = tmp_path / "rumble.db"
    with closing(sqlite3.connect(path)) as plain:
        plain.executescript(db.SCHEMA)
        plain.execute("PRAGMA journal_mode=WAL")
        plain.execute("INSERT INTO chats VALUES ('chat', 'legacy title', 'now', NULL)")
        plain.execute("INSERT INTO messages VALUES ('msg', 'chat', 'user', 'legacy document', 'now')")
        plain.execute("INSERT INTO jobs (id, type, request, created_at) VALUES ('job', 'draft', 'legacy request', 'now')")
        plain.commit()
        expected = {table: plain.execute(f"SELECT * FROM {table}").fetchall()
                    for table in ("chats", "messages", "jobs")}
        # Keep the WAL connection open during migration, like a prior unclean exit.
        try:
            await db.init_db(str(tmp_path))
            for table, rows in expected.items():
                cursor = await db._get_db().execute(f"SELECT * FROM {table}")
                assert [tuple(row) for row in await cursor.fetchall()] == rows
        finally:
            await db.close_db()
    assert not path.read_bytes().startswith(b"SQLite format 3")
    assert sorted(p.name for p in tmp_path.iterdir()) == ["rumble.db"]
    await db.init_db(str(tmp_path))
    try:
        assert (await db.get_messages("chat"))[0]["content"] == "legacy document"
    finally:
        await db.close_db()


async def test_migration_failure_preserves_plaintext_source(tmp_path, monkeypatch):
    path = tmp_path / "rumble.db"
    with closing(sqlite3.connect(path)) as plain:
        plain.execute("CREATE TABLE legacy (content TEXT)")
        plain.execute("INSERT INTO legacy VALUES ('keep on failure')")
        plain.commit()

    def failed_verification(*args):
        raise OSError("simulated verification failure")

    monkeypatch.setattr(db, "_keyed_connection", failed_verification)
    with pytest.raises(OSError, match="simulated verification failure"):
        await db.init_db(str(tmp_path))
    with closing(sqlite3.connect(path)) as plain:
        assert plain.execute("SELECT * FROM legacy").fetchall() == [("keep on failure",)]
    with pytest.raises(RuntimeError, match="not initialized"):
        db._get_db()
    assert sorted(p.name for p in tmp_path.iterdir()) == ["rumble.db"]


async def test_migration_overwrites_old_plaintext_inode(tmp_path, monkeypatch):
    path = tmp_path / "rumble.db"
    with closing(sqlite3.connect(path)) as plain:
        plain.execute("CREATE TABLE legacy (content TEXT)")
        plain.execute("INSERT INTO legacy VALUES ('erase this old inode')")
        plain.commit()
    original_remove = db._secure_remove
    erased = []

    def observe_erasure(backup):
        if backup.exists():
            with backup.open("rb") as old_inode:
                original_remove(backup)
                erased.append(old_inode.read())
        else:
            original_remove(backup)

    monkeypatch.setattr(db, "_secure_remove", observe_erasure)
    await db.init_db(str(tmp_path))
    await db.close_db()
    assert len(erased) == 1
    assert erased[0] and not any(erased[0])


async def test_interrupted_migration_recovers_source(tmp_path):
    path = tmp_path / "rumble.db"
    backup = path.with_name(path.name + ".migrating-sqlcipher-plaintext")
    with closing(sqlite3.connect(backup)) as plain:
        plain.execute("CREATE TABLE legacy (content TEXT)")
        plain.execute("INSERT INTO legacy VALUES ('recover me')")
        plain.commit()
    await db.init_db(str(tmp_path))
    try:
        cursor = await db._get_db().execute("SELECT * FROM legacy")
        assert [tuple(row) for row in await cursor.fetchall()] == [("recover me",)]
        assert not backup.exists()
    finally:
        await db.close_db()
