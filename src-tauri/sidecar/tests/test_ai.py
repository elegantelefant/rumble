# ABOUTME: Tests for stateless AI endpoints (clarify, translate, summarise, chat_title).
# ABOUTME: Validates request parsing, LLM dispatch, JSON response structure.


async def test_clarify_returns_structured_response(client):
    resp = await client.post("/clarify", json={"ask": "what is consideration"})
    assert resp.status_code == 200
    data = resp.json()
    assert "clarified_ask" in data
    assert "Clarified:" in data["clarified_ask"]
    assert isinstance(data["questions"], list)


async def test_clarify_with_context(client):
    resp = await client.post("/clarify", json={
        "ask": "what is consideration",
        "context": {"jurisdiction": "SG"},
        "document_type": "contract",
    })
    assert resp.status_code == 200
    assert "clarified_ask" in resp.json()


async def test_clarify_empty_ask_rejected(client):
    resp = await client.post("/clarify", json={"ask": ""})
    assert resp.status_code == 422


async def test_chat_title_generate(client):
    resp = await client.post("/chat_title/generate", json={
        "messages": ["What is contract law?", "It is a body of law..."],
    })
    assert resp.status_code == 200
    assert resp.json()["title"] == "Generated Title"


async def test_chat_title_empty_messages_rejected(client):
    resp = await client.post("/chat_title/generate", json={"messages": []})
    assert resp.status_code == 422


async def test_translate_returns_translated_text(client):
    resp = await client.post("/translate", json={
        "text": "hello world",
        "target_lang": "es",
    })
    assert resp.status_code == 200
    data = resp.json()
    assert "translated_text" in data
    assert "Translated:" in data["translated_text"]


async def test_translate_missing_target_lang_rejected(client):
    resp = await client.post("/translate", json={"text": "hello"})
    assert resp.status_code == 422


async def test_summarise_document(client):
    resp = await client.post("/summarise/document", json={
        "text": "This agreement is entered into between Party A and Party B...",
    })
    assert resp.status_code == 200
    data = resp.json()
    assert "summary" in data
    assert isinstance(data.get("key_points"), list)


async def test_summarise_chat(client):
    resp = await client.post("/summarise/chat", json={
        "messages": [
            {"role": "user", "content": "What is tort law?"},
            {"role": "assistant", "content": "Tort law deals with civil wrongs."},
        ],
    })
    assert resp.status_code == 200
    data = resp.json()
    assert "summary" in data


async def test_summarise_search(client):
    resp = await client.post("/summarise/search", json={
        "query": "contract breach remedies",
        "results": [
            {"id": "doc-1", "title": "Breach of Contract Overview"},
            {"id": "doc-2", "title": "Remedies for Breach"},
        ],
    })
    assert resp.status_code == 200
    data = resp.json()
    assert "summary" in data
    assert isinstance(data.get("citations"), list)
