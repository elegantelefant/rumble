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
from dataclasses import dataclass
from pathlib import Path

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse

from services.db import close_db, init_db
from services.jobs import shutdown as shutdown_jobs

logger = logging.getLogger(__name__)

SECRET_HEADER = "x-rumble-secret"
PIDFILE_NAME = "sidecar.pid"
SIDECAR_EXE_NAME = "rumble-sidecar"
# What an orphan is reparented to once its host is gone: PID 1 (launchd on macOS,
# init/systemd on Linux) or, on Ubuntu desktops, the `systemd --user` subreaper.
INIT_PID = 1
ORPHAN_REAPER_NAMES = frozenset({"launchd", "init", "systemd"})
REAP_TERM_WAIT_SECONDS = 2.0
REAP_KILL_WAIT_SECONDS = 1.0
REAP_POLL_SECONDS = 0.05
PS_TIMEOUT_SECONDS = 2
STDIN_WATCHER_THREAD_NAME = "rumble-stdin-watcher"


@dataclass(frozen=True)
class _Process:
    pid: int
    ppid: int
    name: str
    args: str


def _pidfile_path(data_dir: str | None) -> Path | None:
    """None when there's no stable data dir (e.g. a manual dev run with no --data-dir) —
    there's nowhere durable to put it, and nothing from a prior run to reap."""
    return Path(data_dir) / PIDFILE_NAME if data_dir else None


def _ps_rows(columns: str, fields: int) -> list[list[str]]:
    output = subprocess.run(
        ["ps", "-A", "-o", columns],
        capture_output=True,
        text=True,
        timeout=PS_TIMEOUT_SECONDS,
    ).stdout
    rows = (line.split(None, fields - 1) for line in output.splitlines())
    return [row for row in rows if len(row) == fields]


def _process_table() -> dict[int, _Process]:
    """Two `ps` calls joined by pid, each with its free-text column last and alone.
    The name comes from `comm`, not the first word of `args`: the real bundle
    path is ".../Elefant - Rumble.app/Contents/MacOS/rumble-sidecar", which has
    spaces in it, so splitting args on whitespace would cut it apart."""
    args_by_pid = {int(pid): args for pid, args in _ps_rows("pid=,args=", 2)}
    return {
        int(pid): _Process(int(pid), int(ppid), os.path.basename(comm), args_by_pid.get(int(pid), ""))
        for pid, ppid, comm in _ps_rows("pid=,ppid=,comm=", 3)
    }


def _host_of(proc: _Process, table: dict[int, _Process]) -> _Process | None:
    """The nearest ancestor that isn't a rumble-sidecar: a frozen sidecar is a
    PyInstaller bootloader plus its Python child, so the child's host is its
    grandparent. None when the chain breaks because an ancestor already exited."""
    parent = table.get(proc.ppid)
    while parent is not None and parent.name == SIDECAR_EXE_NAME:
        parent = table.get(parent.ppid)
    return parent


def _skip_reason(pid: int, table: dict[int, _Process], data_dir_pattern: re.Pattern) -> str | None:
    """Why `pid` is not a stray of this installation, or None when it is: a
    rumble-sidecar carrying this --data-dir whose host is gone. Nothing is
    signalled unless this returns None, so neither a reused PID nor a live
    sibling instance is touched — `pnpm tauri dev` and the packaged app share
    one data dir. "Host gone" means orphaned to init or a subreaper, not
    `ppid == 1`, which a `systemd --user` subreaper would turn into a no-op."""
    if pid in (os.getpid(), os.getppid()):
        # Both share the rumble-sidecar name and this --data-dir.
        return "it is this sidecar or its own bootloader"
    proc = table.get(pid)
    if proc is None:
        return "it is not running"
    if proc.name != SIDECAR_EXE_NAME:
        return f"its PID was reused by {proc.name!r}"
    if not data_dir_pattern.search(proc.args):
        return "it serves a different data dir"
    host = _host_of(proc, table)
    if host is not None and host.pid != INIT_PID and host.name not in ORPHAN_REAPER_NAMES:
        return f"its host {host.name!r} (pid {host.pid}) is alive"
    return None


def _exited_within(pid: int, seconds: float) -> bool:
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return True
        time.sleep(REAP_POLL_SECONDS)
    return False


def _terminate(pid: int) -> None:
    """SIGTERM, then SIGKILL if it outlives REAP_TERM_WAIT_SECONDS. Startup goes on
    either way — holding it up for someone else's stuck process would be worse —
    but a survivor still holds the shared secret, so it is logged as an error."""
    for sig, wait in ((signal.SIGTERM, REAP_TERM_WAIT_SECONDS), (signal.SIGKILL, REAP_KILL_WAIT_SECONDS)):
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            logger.warning("stray sidecar pid %d exited before %s", pid, sig.name)
            return
        except OSError as e:
            logger.error("cannot signal stray sidecar pid %d, it is still running: %s", pid, e)
            return
        if _exited_within(pid, wait):
            logger.warning("stray sidecar pid %d exited after %s", pid, sig.name)
            return
    logger.error("stray sidecar pid %d survived SIGKILL and still holds the shared secret", pid)


def _read_pidfile(pidfile: Path) -> int | None:
    try:
        return int(pidfile.read_text().strip())
    except FileNotFoundError:
        return None
    except (OSError, ValueError) as e:
        logger.warning("ignoring unreadable sidecar pidfile %s: %s", pidfile, e)
        return None


def _reap_strays(data_dir: str, pidfile: Path) -> None:
    """Candidates are every rumble-sidecar carrying this --data-dir (strays from
    builds before the pidfile existed never wrote one) plus whatever the pidfile
    names (a crashed post-fix sidecar); each must pass _skip_reason first."""
    if sys.platform == "win32":
        # os.kill(pid, 0) is CTRL_C_EVENT there and `ps` doesn't exist.
        logger.warning("stray sidecar reaping is not implemented on Windows; a sidecar left by a crashed host keeps running")
        return
    try:
        table = _process_table()
    except (OSError, subprocess.SubprocessError, ValueError) as e:
        logger.error("cannot list processes, so stray sidecars are not reaped: %s", e)
        return

    # The needle must end at a word boundary: "--data-dir X" must not match
    # inside "--data-dir X-dev" just because X is a prefix of X-dev.
    data_dir_pattern = re.compile(re.escape(f"--data-dir {data_dir}") + r"(?:$|\s)")
    candidates = {
        p.pid for p in table.values() if p.name == SIDECAR_EXE_NAME and data_dir_pattern.search(p.args)
    } - {os.getpid(), os.getppid()}
    pidfile_pid = _read_pidfile(pidfile)
    if pidfile_pid is not None:
        candidates.add(pidfile_pid)

    for pid in sorted(candidates):
        reason = _skip_reason(pid, table, data_dir_pattern)
        if reason is None:
            logger.warning("stray sidecar pid %d found for %s with its host gone; terminating", pid, data_dir)
            _terminate(pid)
        else:
            logger.warning("leaving sidecar pid %d alone: %s", pid, reason)


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
    # Before opening the database: a stray sidecar from a previous run — with
    # or without this fix — still holds the same SQLite file, so it must be gone
    # (or confirmed not ours to kill) before we touch it.
    pidfile = _pidfile_path(app.state.data_dir)
    if pidfile is not None:
        _reap_strays(app.state.data_dir, pidfile)

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
    from routes.extract import router as extract_router
    from routes.health import router as health_router
    from routes.jobs import router as jobs_router

    app.include_router(health_router)
    app.include_router(chat_router)
    app.include_router(ai_router)
    app.include_router(jobs_router)
    app.include_router(extract_router)
    return app
