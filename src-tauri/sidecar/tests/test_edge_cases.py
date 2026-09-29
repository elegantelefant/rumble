# ABOUTME: Edge-case integration tests for sidecar endpoints.
# ABOUTME: Covers missing fields, optional params, parse_llm_json, job lifecycle.

import asyncio
import json

import pytest


# --- Chat edge cases ---


async def test_patch_nonexistent_chat_returns_404(client):
    resp = await client.patch("/chats/nonexistent", json={"title": "New"})
    assert resp.status_code == 404


async def test_send_message_with_model_field(client, chat_id):
    resp = await client.post(
        f"/chats/{chat_id}/message",
        json={"text": "Hello", "model": "gpt-4o"},
    )
    assert resp.status_code == 200
    assert resp.json()["role"] == "assistant"


# --- AI edge cases ---


async def test_clarify_missing_ask_field(client):
    resp = await client.post("/clarify", json={})
    assert resp.status_code == 422


async def test_clarify_with_all_optional_fields(client):
    resp = await client.post("/clarify", json={
        "ask": "what is negligence",
        "context": {"jurisdiction": "US"},
        "document_type": "memo",
        "drafting_style": "formal",
        "parties": [{"name": "Plaintiff", "role": "claimant"}],
        "document_terms": {"term1": "val1"},
        "compressed_references": ["ref1"],
    })
    assert resp.status_code == 200
    assert "clarified_ask" in resp.json()


async def test_translate_empty_text(client):
    resp = await client.post("/translate", json={"text": "", "target_lang": "es"})
    assert resp.status_code == 422


async def test_summarise_document_with_style(client):
    resp = await client.post("/summarise/document", json={
        "text": "This is a long legal document about corporate governance.",
        "style": "detailed",
    })
    assert resp.status_code == 200
    assert "summary" in resp.json()


async def test_summarise_document_empty_text(client):
    resp = await client.post("/summarise/document", json={"text": ""})
    assert resp.status_code == 422


async def test_summarise_chat_with_style(client):
    resp = await client.post("/summarise/chat", json={
        "messages": [
            {"role": "user", "content": "What is IP law?"},
            {"role": "assistant", "content": "IP law covers..."},
        ],
        "style": "brief",
    })
    assert resp.status_code == 200
    assert "summary" in resp.json()


async def test_summarise_search_empty_results_accepted(client):
    """Empty results list is valid — the model has no min_length on results."""
    resp = await client.post("/summarise/search", json={
        "query": "breach remedies",
        "results": [],
    })
    assert resp.status_code == 200
    assert "summary" in resp.json()


# --- Job edge cases ---


async def test_draft_with_all_optional_fields(client):
    resp = await client.post("/draft", json={
        "prompt": "NDA between two parties",
        "document_type": "nda",
        "drafting_style": "formal",
        "parties": [{"name": "Company A", "role": "drafter"}, {"name": "Company B", "role": "counterparty"}],
        "document_terms": {"term": "12 months"},
        "model": "gpt-4o",
    })
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "queued"
    assert data["job_id"]


async def test_review_nonexistent_job_returns_404(client):
    resp = await client.get("/review/nonexistent/result")
    assert resp.status_code == 404


async def test_research_result_nonexistent_returns_404(client):
    resp = await client.get("/research/nonexistent/result")
    assert resp.status_code == 404


async def test_research_with_all_optional_fields(client):
    resp = await client.post("/research", json={
        "question": "What are the elements of negligence?",
        "constraints": {"jurisdiction": "UK"},
        "guidelines": "Focus on recent case law",
        "primary_source": "Donoghue v Stevenson",
        "model": "gpt-4o",
    })
    assert resp.status_code == 200
    assert resp.json()["status"] == "queued"


async def test_job_lifecycle_queued_to_completed(client):
    """Verify the full lifecycle: queued → running → completed."""
    resp = await client.post("/draft", json={"prompt": "Simple NDA"})
    job_id = resp.json()["job_id"]

    # Poll until completed (max 2 seconds)
    for _ in range(20):
        result = await client.get(f"/draft/{job_id}/result")
        data = result.json()
        if data["status"] == "completed":
            assert data["result"] is not None
            return
        await asyncio.sleep(0.1)
    pytest.fail("Job did not complete within timeout")


# --- Malformed job result ---


async def test_malformed_draft_result_returns_502(client):
    """Corrupted JSON in job result column should yield 502."""
    from services import db as svc_db

    job = await svc_db.create_job("draft", "{}")
    # Manually write invalid JSON into result column
    raw_db = svc_db._get_db()
    await raw_db.execute(
        "UPDATE jobs SET status='completed', result='NOT-JSON' WHERE id=?",
        (job["id"],),
    )
    await raw_db.commit()
    resp = await client.get(f"/draft/{job['id']}/result")
    assert resp.status_code == 502
    assert "malformed result" in resp.json()["detail"]


async def test_malformed_research_result_returns_502(client):
    """Corrupted JSON in research result should yield 502."""
    from services import db as svc_db

    job = await svc_db.create_job("research", "{}")
    raw_db = svc_db._get_db()
    await raw_db.execute(
        "UPDATE jobs SET status='completed', result='{{bad' WHERE id=?",
        (job["id"],),
    )
    await raw_db.commit()
    resp = await client.get(f"/research/{job['id']}/result")
    assert resp.status_code == 502


async def test_malformed_review_result_returns_502(client):
    """Corrupted JSON in review result should yield 502."""
    from services import db as svc_db

    job = await svc_db.create_job("review", "{}")
    raw_db = svc_db._get_db()
    await raw_db.execute(
        "UPDATE jobs SET status='completed', result='{{bad' WHERE id=?",
        (job["id"],),
    )
    await raw_db.commit()
    resp = await client.get(f"/review/{job['id']}/result")
    assert resp.status_code == 502
    assert "malformed result" in resp.json()["detail"]


async def test_draft_result_wrong_shape_returns_502(client):
    """Valid JSON that doesn't match DraftResponse should yield 502 (see #46)."""
    from services import db as svc_db

    job = await svc_db.create_job("draft", "{}")
    raw_db = svc_db._get_db()
    payload = json.dumps({
        "employment_agreement": {
            "additional_terms": "Example",
            "document_type": "employment",
            "employee_name": "Tester",
            "position": "Role",
            "salary": 100000,
            "start_date": "2026-09-19",
        }
    })
    await raw_db.execute(
        "UPDATE jobs SET status='completed', result=? WHERE id=?",
        (payload, job["id"]),
    )
    await raw_db.commit()
    resp = await client.get(f"/draft/{job['id']}/result")
    assert resp.status_code == 502
    assert "expected shape" in resp.json()["detail"]


async def test_draft_result_with_bad_warnings_returns_200(client):
    """A wrong-typed optional field (warnings) doesn't 502, and the filtered
    response drops it entirely rather than passing the wrong-typed value to the
    client — DocumentDraftView v-for's over warnings, so a string there would
    render one list item per character instead of failing loudly.
    """
    from services import db as svc_db

    job = await svc_db.create_job("draft", "{}")
    raw_db = svc_db._get_db()
    payload = json.dumps({"draft": "Full contract text.", "warnings": "Check the dates."})
    await raw_db.execute(
        "UPDATE jobs SET status='completed', result=? WHERE id=?",
        (payload, job["id"]),
    )
    await raw_db.commit()
    resp = await client.get(f"/draft/{job['id']}/result")
    assert resp.status_code == 200
    data = resp.json()
    assert data["result"]["draft"] == "Full contract text."
    assert data["result"]["warnings"] is None


async def test_review_result_wrong_shape_returns_502(client):
    """Valid JSON missing the required summary field should yield 502."""
    from services import db as svc_db

    job = await svc_db.create_job("review", "{}")
    raw_db = svc_db._get_db()
    payload = json.dumps({"issues": []})
    await raw_db.execute(
        "UPDATE jobs SET status='completed', result=? WHERE id=?",
        (payload, job["id"]),
    )
    await raw_db.commit()
    resp = await client.get(f"/review/{job['id']}/result")
    assert resp.status_code == 502
    assert "expected shape" in resp.json()["detail"]


async def test_review_result_with_bad_issues_returns_200(client):
    """A wrong-typed optional field (issues) is dropped, not fatal — the summary is still usable."""
    from services import db as svc_db

    job = await svc_db.create_job("review", "{}")
    raw_db = svc_db._get_db()
    payload = json.dumps({
        "summary": "Overall the agreement is standard.",
        "issues": "Check the dates.",
    })
    await raw_db.execute(
        "UPDATE jobs SET status='completed', result=? WHERE id=?",
        (payload, job["id"]),
    )
    await raw_db.commit()
    resp = await client.get(f"/review/{job['id']}/result")
    assert resp.status_code == 200
    data = resp.json()
    assert data["result"]["summary"] == "Overall the agreement is standard."


async def test_review_result_with_extra_issue_fields_returns_200(client):
    """Unknown keys and an out-of-range kind inside a ReviewIssue item shouldn't 502."""
    from services import db as svc_db

    job = await svc_db.create_job("review", "{}")
    raw_db = svc_db._get_db()
    payload = json.dumps({
        "summary": "Overall the agreement is standard.",
        "issues": [
            {
                "kind": "formatting",
                "severity": "high",
                "message": "Inconsistent heading style",
            }
        ],
    })
    await raw_db.execute(
        "UPDATE jobs SET status='completed', result=? WHERE id=?",
        (payload, job["id"]),
    )
    await raw_db.commit()
    resp = await client.get(f"/review/{job['id']}/result")
    assert resp.status_code == 200
    data = resp.json()
    assert data["result"]["issues"]


async def test_research_result_with_name_in_source_returns_200(client):
    """A source using 'name' plus an unknown key should still pass through as 200."""
    from services import db as svc_db

    job = await svc_db.create_job("research", "{}")
    raw_db = svc_db._get_db()
    payload = json.dumps({
        "result": "Negligence requires duty, breach, causation, and damage.",
        "sources": [
            {
                "name": "Donoghue v Stevenson",
                "unknown_field": "should be dropped",
                "published_at": "2026-01-15T00:00:00+00:00",
            }
        ],
    })
    await raw_db.execute(
        "UPDATE jobs SET status='completed', result=? WHERE id=?",
        (payload, job["id"]),
    )
    await raw_db.commit()
    resp = await client.get(f"/research/{job['id']}/result")
    assert resp.status_code == 200
    data = resp.json()
    assert data["sources"]
    assert data["sources"][0]["title"] == "Donoghue v Stevenson"


async def test_research_result_with_one_malformed_source_keeps_valid_ones(client):
    """One malformed source item is dropped; the other, valid sources survive."""
    from services import db as svc_db

    job = await svc_db.create_job("research", "{}")
    raw_db = svc_db._get_db()
    payload = json.dumps({
        "result": "Negligence requires duty, breach, causation, and damage.",
        "sources": [
            {"id": "src-1", "title": "Donoghue v Stevenson"},
            {"id": "src-2", "title": "Caparo v Dickman"},
            {"id": 123, "title": "Malformed — id is not a string"},
        ],
    })
    await raw_db.execute(
        "UPDATE jobs SET status='completed', result=? WHERE id=?",
        (payload, job["id"]),
    )
    await raw_db.commit()
    resp = await client.get(f"/research/{job['id']}/result")
    assert resp.status_code == 200
    data = resp.json()
    titles = [source["title"] for source in data["sources"]]
    assert titles == ["Donoghue v Stevenson", "Caparo v Dickman"]


async def test_research_result_with_extra_key_in_source_field_returns_200(client):
    """An unknown key inside SearchResult.source shouldn't 502 either."""
    from services import db as svc_db

    job = await svc_db.create_job("research", "{}")
    raw_db = svc_db._get_db()
    payload = json.dumps({
        "result": "Negligence requires duty, breach, causation, and damage.",
        "sources": [
            {
                "id": "src-1",
                "title": "Donoghue v Stevenson",
                "source": {
                    "name": "House of Lords",
                    "url": "https://example.com",
                    "type": "case law",
                },
                "metadata": {"jurisdiction": "UK", "confidence": 0.9},
            }
        ],
    })
    await raw_db.execute(
        "UPDATE jobs SET status='completed', result=? WHERE id=?",
        (payload, job["id"]),
    )
    await raw_db.commit()
    resp = await client.get(f"/research/{job['id']}/result")
    assert resp.status_code == 200
    data = resp.json()
    assert data["sources"][0]["source"]["name"] == "House of Lords"
    # metadata is dict[str, Any]: the nested-submodel loop inspects it (it's a dict)
    # but must not filter it, since _nested_submodel keys off the annotation, not
    # the value, and dict[str, Any] isn't a BaseModel.
    assert data["sources"][0]["metadata"] == {"jurisdiction": "UK", "confidence": 0.9}


# --- _parse_llm_json tests (via endpoint behavior) ---
# These are tested indirectly through the AI endpoints.
# The mock returns clean JSON, so these pass.
# Direct _parse_llm_json tests are below.


def test_parse_llm_json_clean():
    from routes.ai import _parse_llm_json
    result = _parse_llm_json('{"key": "value"}')
    assert result == {"key": "value"}


def test_parse_llm_json_with_markdown_fences():
    from routes.ai import _parse_llm_json
    raw = '```json\n{"key": "value"}\n```'
    result = _parse_llm_json(raw)
    assert result == {"key": "value"}


def test_parse_llm_json_with_bare_fences():
    from routes.ai import _parse_llm_json
    raw = '```\n{"key": "value"}\n```'
    result = _parse_llm_json(raw)
    assert result == {"key": "value"}


def test_parse_llm_json_invalid_raises_502():
    from fastapi import HTTPException
    from routes.ai import _parse_llm_json
    with pytest.raises(HTTPException) as exc_info:
        _parse_llm_json("not valid json at all")
    assert exc_info.value.status_code == 502
    assert "unparseable" in exc_info.value.detail


def test_parse_llm_json_whitespace():
    from routes.ai import _parse_llm_json
    result = _parse_llm_json('  \n  {"key": "value"}  \n  ')
    assert result == {"key": "value"}
