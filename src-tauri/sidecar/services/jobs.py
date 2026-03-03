# ABOUTME: Async job runner for heavy LLM tasks (draft, review, research).
# ABOUTME: Spawns background asyncio tasks, persists status/results to SQLite.

import asyncio
import json
from collections.abc import Coroutine

from services import db


async def start_job(job_id: str, coro: Coroutine) -> None:
    """Spawn a background task that runs the coroutine and writes results to the DB."""
    asyncio.create_task(_run_job(job_id, coro))


async def _run_job(job_id: str, coro: Coroutine) -> None:
    """Execute the coroutine, updating job status as it progresses."""
    try:
        await db.update_job_status(job_id, "running")
        result = await coro
        await db.set_job_result(job_id, result=json.dumps(result) if isinstance(result, dict) else result)
    except Exception as exc:
        await db.set_job_result(job_id, error=str(exc))
