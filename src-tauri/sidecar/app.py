# ABOUTME: FastAPI application factory for the rumble sidecar.
# ABOUTME: Wires up lifespan (DB init/close, pidfile, stdin watcher), shared-secret auth, and route modules.
import asyncio
import logging
import os
import re
import secrets
import signal
import subprocess
import sys
import threading
import time
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse

from services.db import close_db, init_db
from services.jobs import shutdown as shutdown_jobs

logger = logging.getLogger(__name__)

SECRET_HEADER = "x-rumble-secret"
PIDFILE_NAME = "sidecar.pid"
SIDECAR_EXE_NAME = "rumble-sidecar"
REAP_WAIT_SECONDS = 2.0
STDIN_WATCHER_THREAD_NAME = "rumble-stdin-watcher"


def _pidfile_path(data_dir: str | None) -> Path | None:
    """None when there's no stable data dir (e.g. a manual dev run with no --data-dir) —
    there's nowhere durable to put it, and nothing from a prior run to reap."""
    return Path(data_dir) / PIDFILE_NAME if data_dir else None


def _terminate_and_wait(pid: int) -> None:
    """SIGTERM a confirmed stray and wait briefly for it to actually go away —
    best-effort; if it hasn't exited by the deadline we still continue, since
    holding up our own startup indefinitely for someone else's stuck process
    would be worse than a rare double-run."""
    try:
        os.kill(pid, signal.SIGTERM)
    except OSError:
        return

    deadline = time.monotonic() + REAP_WAIT_SECONDS
    while time.monotonic() < deadline:
        try:
            os.kill(pid, 0)
        except OSError:
            return  # gone
        time.sleep(0.05)


def _reap_stray_sidecar(pidfile: Path) -> None:
    """Kill a sidecar left running from before this fix (or a crash). Never trust the
    PID alone — PIDs get reused — so only act if a live process at that PID is
    actually named rumble-sidecar. Runs before init_db, since a stray still holds
    the same SQLite database.

    This only catches a stray that itself ran with this fix in place (it's the
    one that wrote the pidfile) — e.g. a post-fix crash where the sidecar's
    parent hasn't been reparented to PID 1 yet. It can't find a stray from a
    build before this fix, since one was never written; see
    _reap_orphaned_sidecars for that case."""
    try:
        pid = int(pidfile.read_text().strip())
    except (OSError, ValueError):
        return

    if pid in (os.getpid(), os.getppid()):
        # A reused PID could point at this very process, or at its own PyInstaller
        # bootloader parent — both share the rumble-sidecar executable name, so the
        # name check below can't tell them apart from an actual stray.
        return

    try:
        os.kill(pid, 0)
    except OSError:
        return  # not alive; the pidfile is just stale and will be overwritten below

    try:
        comm = subprocess.run(
            ["ps", "-p", str(pid), "-o", "comm="],
            capture_output=True,
            text=True,
            timeout=2,
        ).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return

    if os.path.basename(comm) != SIDECAR_EXE_NAME:
        return  # the PID was reused by something else; leave it running

    _terminate_and_wait(pid)


def _args_by_pid() -> dict[int, str]:
    """A separate `ps` call from the name/parent lookup: args is the last column
    here too, but it's the *only* column, so there's nothing after it that a
    split could misalign — unlike comm, which shares a line with pid/ppid."""
    try:
        output = subprocess.run(
            ["ps", "-A", "-o", "pid=,args="],
            capture_output=True,
            text=True,
            timeout=2,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return {}

    result = {}
    for line in output.splitlines():
        parts = line.split(None, 1)
        if len(parts) != 2:
            continue
        pid_str, args = parts
        try:
            result[int(pid_str)] = args
        except ValueError:
            continue
    return result


def _find_orphaned_sidecars(data_dir: str) -> list[int]:
    """Finds strays from before this fix — these never wrote a pidfile at all, so
    they're what's actually running on users' machines today, and the pidfile
    check above can't see them. Matched by argv, not PID: a process named
    rumble-sidecar, reparented to PID 1 (its own host already exited), whose
    --data-dir is exactly this sidecar's.

    The name comes from `comm`, not the first word of `args`: the real bundle
    path is ".../Elefant - Rumble.app/Contents/MacOS/rumble-sidecar", which has
    spaces in it, so splitting args on whitespace would cut it apart. `comm` is
    a separate `ps` column, joined here by pid, specifically so its own spaces
    can't be confused with a field boundary."""
    try:
        comm_output = subprocess.run(
            ["ps", "-A", "-o", "pid=,ppid=,comm="],
            capture_output=True,
            text=True,
            timeout=2,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return []

    args_by_pid = _args_by_pid()
    # The needle must end at a word boundary: "--data-dir X" must not match
    # inside "--data-dir X-dev" just because X is a prefix of X-dev.
    needle_pattern = re.compile(re.escape(f"--data-dir {data_dir}") + r"(?:$|\s)")

    matches = []
    for line in comm_output.splitlines():
        parts = line.split(None, 2)
        if len(parts) != 3:
            continue
        pid_str, ppid_str, comm = parts
        try:
            pid, ppid = int(pid_str), int(ppid_str)
        except ValueError:
            continue

        if pid in (os.getpid(), os.getppid()):
            continue
        if ppid != 1:
            continue  # not an orphan — leave whatever still owns it alone
        if os.path.basename(comm) != SIDECAR_EXE_NAME:
            continue

        args = args_by_pid.get(pid)
        if args is None or not needle_pattern.search(args):
            continue  # a different sidecar instance's data dir, or gone by now

        matches.append(pid)
    return matches


def _reap_orphaned_sidecars(data_dir: str) -> None:
    for pid in _find_orphaned_sidecars(data_dir):
        _terminate_and_wait(pid)


def _write_pidfile(pidfile: Path) -> None:
    pidfile.parent.mkdir(parents=True, exist_ok=True)
    pidfile.write_text(str(os.getpid()))


def _remove_pidfile(pidfile: Path) -> None:
    pidfile.unlink(missing_ok=True)


def _watch_stdin_for_eof(loop: asyncio.AbstractEventLoop, server) -> None:
    """Runs on its own daemon thread, not a run_in_executor pool thread: pool threads
    aren't daemonic, so a normal SIGTERM shutdown would hang shutting down the
    executor while this blocking read waits for a stdin that will never close.
    On EOF (the host's end of the pipe closed — it died), hand the graceful-shutdown
    signal back to the event loop rather than touching server state from this thread.
    """
    if sys.stdin is None:
        # Can happen under some launch configurations (e.g. a windowed subprocess
        # with no stdio at all) — nothing to watch, so exit instead of dying with
        # an unhandled AttributeError on a background thread.
        logger.warning("stdin watcher started with sys.stdin is None; not watching")
        return

    while True:
        chunk = sys.stdin.buffer.read(1)
        if chunk == b"":
            loop.call_soon_threadsafe(setattr, server, "should_exit", True)
            return


@asynccontextmanager
async def lifespan(app: FastAPI):
    # Both before opening the database: a stray sidecar from a previous run — with
    # or without this fix — still holds the same SQLite file, so it must be gone
    # (or confirmed not ours to kill) before we touch it.
    if app.state.data_dir:
        _reap_orphaned_sidecars(app.state.data_dir)

    pidfile = _pidfile_path(app.state.data_dir)
    if pidfile is not None:
        _reap_stray_sidecar(pidfile)

    await init_db(app.state.data_dir)

    if pidfile is not None:
        _write_pidfile(pidfile)

    if app.state.watch_stdin:
        loop = asyncio.get_running_loop()
        threading.Thread(
            target=_watch_stdin_for_eof,
            args=(loop, app.state.server),
            daemon=True,
            name=STDIN_WATCHER_THREAD_NAME,
        ).start()

    try:
        yield
    finally:
        # try/finally, not bare sequential calls: if the request/test code inside
        # the `with` block raises, @asynccontextmanager resumes this generator via
        # athrow() at the yield point above. Without a finally, that exception
        # would skip straight past close_db() — leaking the DB's own non-daemon
        # worker thread and never removing the pidfile.
        await shutdown_jobs()
        await close_db()

        if pidfile is not None:
            _remove_pidfile(pidfile)


def create_app(
    data_dir: str | None = None,
    secret: str | None = None,
    dev: bool = False,
    watch_stdin: bool = False,
) -> FastAPI:
    if not secret and not dev:
        raise ValueError("create_app requires either a secret or dev=True")

    app = FastAPI(title="rumble-sidecar", version="0.1.0", lifespan=lifespan)
    app.state.data_dir = data_dir
    app.state.secret = secret
    app.state.watch_stdin = watch_stdin
    # Set by main.py once the real uvicorn.Server exists; only read when watch_stdin
    # is True, which only main.py's real spawn path ever sets.
    app.state.server = None

    if secret:

        @app.middleware("http")
        async def verify_shared_secret(request: Request, call_next):
            provided = request.headers.get(SECRET_HEADER) or ""
            if not secrets.compare_digest(provided, app.state.secret):
                return JSONResponse(
                    status_code=401,
                    content={"detail": "missing or invalid shared secret"},
                )
            return await call_next(request)
    else:
        logger.warning(
            "Sidecar running with --dev and no shared secret: every endpoint is UNAUTHENTICATED. "
            "Never use --dev outside local development."
        )

    from routes.ai import router as ai_router
    from routes.chat import router as chat_router
    from routes.health import router as health_router
    from routes.jobs import router as jobs_router

    app.include_router(health_router)
    app.include_router(chat_router)
    app.include_router(ai_router)
    app.include_router(jobs_router)
    return app
