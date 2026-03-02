# ABOUTME: Unit tests for the aiosqlite database service.
# ABOUTME: Verifies chat CRUD, message storage, cascade delete, and ordering.

import pytest_asyncio

from services import db


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
