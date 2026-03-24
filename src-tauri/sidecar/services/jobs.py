# ABOUTME: Async job runner for heavy LLM tasks (draft, review, research).
# ABOUTME: Spawns background asyncio tasks, persists status/results to SQLite.

import asyncio
import json
import logging
from collections.abc import Coroutine

from services import db

logger = logging.getLogger(__name__)

JOB_TIMEOUT_SECONDS = 300

_tasks: dict[str, asyncio.Task] = {}


async def start_job(job_id: str, coro: Coroutine) -> None:
    """Spawn a background task that runs the coroutine and writes results to the DB."""
    task = asyncio.create_task(_run_job(job_id, coro))
    _tasks[job_id] = task
    task.add_done_callback(lambda _t: _tasks.pop(job_id, None))


async def _run_job(job_id: str, coro: Coroutine) -> None:
    """Execute the coroutine with timeout, updating job status as it progresses."""
    try:
        await db.update_job_status(job_id, "running")
        result = await asyncio.wait_for(coro, timeout=JOB_TIMEOUT_SECONDS)
        await db.set_job_result(job_id, result=json.dumps(result) if isinstance(result, dict) else result)
    except asyncio.TimeoutError:
        logger.error("Job %s timed out after %ds", job_id, JOB_TIMEOUT_SECONDS)
        await db.set_job_result(job_id, error=f"timed out after {JOB_TIMEOUT_SECONDS}s")
    except asyncio.CancelledError:
        logger.info("Job %s cancelled during shutdown", job_id)
        await db.set_job_result(job_id, error="cancelled during shutdown")
    except Exception as exc:
        logger.exception("Job %s failed", job_id)
        await db.set_job_result(job_id, error=str(exc))


async def shutdown() -> None:
    """Cancel all running tasks and wait for them to finish."""
    if not _tasks:
        return
    logger.info("Shutting down %d running job(s)", len(_tasks))
    for task in _tasks.values():
        task.cancel()
    await asyncio.gather(*_tasks.values(), return_exceptions=True)
    _tasks.clear()
