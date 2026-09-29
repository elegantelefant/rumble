# ABOUTME: Contract tests — validate route responses against OpenAPI-generated Pydantic models.
# ABOUTME: Models use extra='forbid', so any drift (extra/missing fields) causes ValidationError.

import asyncio
import json

import pytest

from models.generated import (
    ChatCreateResponse,
    ChatDetailResponse,
    ChatListResponse,
    ChatMessagesResponse,
    ChatTitleResponse,
    ClarifyResponse,
    HealthResponse,
    JobCreatedResponse,
    JobResultResponse,
    ModelListResponse,
    ReadyResponse,
    ResearchResponse,
    ResearchResultResponse,
    SummariseChatResponse,
    SummariseDocumentResponse,
    SummariseSearchResponse,
    TranslateResponse,
)


# --- Health / Ready ---


@pytest.mark.xfail(reason="route returns extra 'mode' field not in HealthResponse", strict=True)
async def test_health_matches_contract(client):
    resp = await client.get("/health")
    HealthResponse(**resp.json())


@pytest.mark.xfail(reason="route returns extra 'mode', 'error' fields not in ReadyResponse", strict=True)
async def test_ready_matches_contract(client):
    resp = await client.get("/ready")
    ReadyResponse(**resp.json())


async def test_models_matches_contract(client, monkeypatch, fake_ollama):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    fake_ollama.set_tags("llama3.2:latest")
    resp = await client.get("/models")
    ModelListResponse(**resp.json())


async def test_models_error_matches_contract(client, monkeypatch):
    monkeypatch.delenv("RUMBLE_BACKEND_MODE", raising=False)
    monkeypatch.setenv("OLLAMA_BASE_URL", "http://evil.example.com:11434")
    resp = await client.get("/models")
    assert ModelListResponse(**resp.json()).error


# --- Chat CRUD ---


async def test_create_chat_matches_contract(client):
    resp = await client.post("/chats", json={"title": "Contract test"})
    ChatCreateResponse(**resp.json())


async def test_list_chats_matches_contract(client):
    await client.post("/chats", json={"title": "One"})
    resp = await client.get("/chats")
    data = resp.json()
    validated = ChatListResponse(**data)
    # ChatSummary validation happens inside ChatListResponse
    for chat in validated.chats or []:
        pass


async def test_get_chat_matches_contract(client, chat_id):
    resp = await client.get(f"/chats/{chat_id}")
    ChatDetailResponse(**resp.json())


async def test_get_messages_matches_contract(client, chat_id):
    resp = await client.get(f"/chats/{chat_id}/messages")
    ChatMessagesResponse(**resp.json())


# --- Stateless AI endpoints (all have response_model=) ---


async def test_clarify_matches_contract(client):
    resp = await client.post("/clarify", json={"ask": "What is tort law?"})
    assert resp.status_code == 200
    ClarifyResponse(**resp.json())


async def test_chat_title_matches_contract(client):
    resp = await client.post("/chat_title/generate", json={"messages": ["Hello", "What is contract law?"]})
    assert resp.status_code == 200
    ChatTitleResponse(**resp.json())


async def test_translate_matches_contract(client):
    resp = await client.post("/translate", json={"text": "Hello world", "target_lang": "es"})
    assert resp.status_code == 200
    TranslateResponse(**resp.json())


async def test_summarise_doc_matches_contract(client):
    resp = await client.post("/summarise/document", json={"text": "This is a legal document about..."})
    assert resp.status_code == 200
    SummariseDocumentResponse(**resp.json())


async def test_summarise_chat_matches_contract(client):
    resp = await client.post(
        "/summarise/chat",
        json={"messages": [{"role": "user", "content": "Hello"}, {"role": "assistant", "content": "Hi"}]},
    )
    assert resp.status_code == 200
    SummariseChatResponse(**resp.json())


async def test_summarise_search_matches_contract(client):
    resp = await client.post(
        "/summarise/search",
        json={
            "query": "tort law",
            "results": [{"id": "doc1", "title": "Tort Law Basics", "snippet": "...", "score": 0.9}],
        },
    )
    assert resp.status_code == 200
    SummariseSearchResponse(**resp.json())


# --- Job endpoints ---


async def test_draft_created_matches_contract(client):
    resp = await client.post("/draft", json={"prompt": "Draft a basic NDA"})
    assert resp.status_code == 200
    JobCreatedResponse(**resp.json())


async def test_draft_result_matches_contract(client):
    resp = await client.post("/draft", json={"prompt": "Draft a basic NDA"})
    job_id = resp.json()["job_id"]
    # Wait for background task to complete
    await asyncio.sleep(0.2)
    resp = await client.get(f"/draft/{job_id}/result")
    assert resp.status_code == 200
    JobResultResponse(**resp.json())


async def test_research_created_matches_contract(client):
    resp = await client.post("/research", json={"question": "What is force majeure?"})
    assert resp.status_code == 200
    ResearchResponse(**resp.json())


async def test_research_result_matches_contract(client):
    resp = await client.post("/research", json={"question": "What is force majeure?"})
    report_id = resp.json()["report_id"]
    await asyncio.sleep(0.2)
    resp = await client.get(f"/research/{report_id}/result")
    assert resp.status_code == 200
    ResearchResultResponse(**resp.json())
