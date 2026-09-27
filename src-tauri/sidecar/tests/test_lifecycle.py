# ABOUTME: Tests for the stdin-EOF watcher and pidfile reaping in app.py's lifespan.
# ABOUTME: Covers the watch_stdin gate, EOF-driven shutdown, and stray-process reaping.
import asyncio
import os
import signal
import subprocess
import sys
import threading
from types import SimpleNamespace

import pytest

from app import (
    STDIN_WATCHER_THREAD_NAME,
    _reap_orphaned_sidecars,
    _reap_stray_sidecar,
    _watch_stdin_for_eof,
    _write_pidfile,
    create_app,
    lifespan,
)


class _EOFStdin:
    class buffer:
        @staticmethod
        def read(_n):
            return b""


async def test_watch_stdin_eof_triggers_graceful_shutdown(monkeypatch):
    monkeypatch.setattr(sys, "stdin", _EOFStdin())
    loop = asyncio.get_running_loop()
    server = SimpleNamespace(should_exit=False)

    threading.Thread(
        target=_watch_stdin_for_eof, args=(loop, server), daemon=True
    ).start()

    for _ in range(50):
        if server.should_exit:
            break
        await asyncio.sleep(0.01)

    assert server.should_exit is True


def _stdin_watcher_running() -> bool:
    return any(t.name == STDIN_WATCHER_THREAD_NAME for t in threading.enumerate())


async def test_lifespan_ignores_closed_stdin_when_not_watching(tmp_path, monkeypatch):
    monkeypatch.setattr(sys, "stdin", _EOFStdin())

    application = create_app(data_dir=str(tmp_path), watch_stdin=False)
    application.state.server = SimpleNamespace(should_exit=False)

    async with lifespan(application):
        await asyncio.sleep(0.05)
        # A raw thread count would also catch aiosqlite's own non-daemon worker
        # thread, which init_db always starts regardless of watch_stdin — assert
        # on the watcher's name specifically, not the count.
        assert not _stdin_watcher_running()
        assert application.state.server.should_exit is False


async def test_lifespan_cleans_up_even_when_the_body_raises(tmp_path):
    before = {t.ident for t in threading.enumerate()}

    application = create_app(data_dir=str(tmp_path), watch_stdin=False)
    pidfile = tmp_path / "sidecar.pid"

    with pytest.raises(RuntimeError):
        async with lifespan(application):
            assert pidfile.exists()
            raise RuntimeError("boom")

    assert not pidfile.exists()

    # close_db() signals aiosqlite's worker thread to stop but doesn't guarantee
    # it has fully exited the instant it returns — poll briefly rather than
    # asserting once and risking a flaky race.
    after = {t.ident for t in threading.enumerate()}
    for _ in range(50):
        after = {t.ident for t in threading.enumerate()}
        if after <= before:
            break
        await asyncio.sleep(0.02)
    assert after <= before  # aiosqlite's worker thread didn't outlive close_db()


def test_reap_leaves_live_non_sidecar_process_alone(tmp_path):
    proc = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(5)"])
    try:
        pidfile = tmp_path / "sidecar.pid"
        pidfile.write_text(str(proc.pid))

        _reap_stray_sidecar(pidfile)

        assert proc.poll() is None  # reap left our test process running
    finally:
        proc.terminate()
        proc.wait(timeout=5)


def test_reap_replaces_pidfile_for_dead_pid(tmp_path):
    proc = subprocess.Popen([sys.executable, "-c", "pass"])
    proc.wait()
    dead_pid = proc.pid

    pidfile = tmp_path / "sidecar.pid"
    pidfile.write_text(str(dead_pid))

    _reap_stray_sidecar(pidfile)  # no-op on a dead PID, must not raise
    _write_pidfile(pidfile)

    assert pidfile.read_text().strip() == str(os.getpid())


# The real installed path — has spaces in it, which is exactly what a
# first-word-of-args split would get wrong.
SIDECAR_BUNDLE_PATH = "/Applications/Elefant - Rumble.app/Contents/MacOS/rumble-sidecar"


def _fake_ps_two_calls(comm_output, args_output):
    """_find_orphaned_sidecars makes two separate `ps` calls (one for
    pid/ppid/comm, one for pid/args) — dispatch on which columns were asked
    for rather than call order, so the fake doesn't depend on that detail."""

    def _run(cmd, **kwargs):
        o_value = cmd[cmd.index("-o") + 1]
        if "comm=" in o_value:
            return SimpleNamespace(stdout=comm_output)
        return SimpleNamespace(stdout=args_output)

    return _run


def _fake_kill_dies_immediately(calls):
    """Records every os.kill call; a SIGTERM succeeds, and the liveness probe
    that follows reports the process already gone, so _terminate_and_wait's
    poll loop exits on its first check instead of really sleeping."""

    def _fake_kill(pid, sig):
        calls.append((pid, sig))
        if sig != 0:
            return
        raise OSError

    return _fake_kill


def test_reap_orphaned_by_args_kills_matching_orphan(tmp_path, monkeypatch):
    data_dir = str(tmp_path / "Library" / "Application Support" / "com.ielegante.rumble")
    target_pid = 54321
    monkeypatch.setattr(
        subprocess,
        "run",
        _fake_ps_two_calls(
            comm_output=f"{target_pid} 1 {SIDECAR_BUNDLE_PATH}",
            args_output=(
                f"{target_pid} {SIDECAR_BUNDLE_PATH} "
                f"--port 11435 --data-dir {data_dir}"
            ),
        ),
    )

    calls = []
    monkeypatch.setattr(os, "kill", _fake_kill_dies_immediately(calls))

    _reap_orphaned_sidecars(data_dir)

    assert (target_pid, signal.SIGTERM) in calls


def test_reap_orphaned_by_args_leaves_different_data_dir_alone(tmp_path, monkeypatch):
    data_dir = str(tmp_path / "Library" / "Application Support" / "com.ielegante.rumble")
    other_dir = str(tmp_path / "Library" / "Application Support" / "com.other.app")
    monkeypatch.setattr(
        subprocess,
        "run",
        _fake_ps_two_calls(
            comm_output=f"54322 1 {SIDECAR_BUNDLE_PATH}",
            args_output=(
                f"54322 {SIDECAR_BUNDLE_PATH} --port 11435 --data-dir {other_dir}"
            ),
        ),
    )

    calls = []
    monkeypatch.setattr(os, "kill", _fake_kill_dies_immediately(calls))

    _reap_orphaned_sidecars(data_dir)

    assert calls == []


def test_reap_orphaned_by_args_leaves_prefix_sharing_data_dir_alone(tmp_path, monkeypatch):
    # "com.ielegante.rumble-dev" starts with "com.ielegante.rumble" — a plain
    # substring check on the needle would wrongly match this.
    data_dir = str(tmp_path / "Library" / "Application Support" / "com.ielegante.rumble")
    lookalike_dir = str(
        tmp_path / "Library" / "Application Support" / "com.ielegante.rumble-dev"
    )
    monkeypatch.setattr(
        subprocess,
        "run",
        _fake_ps_two_calls(
            comm_output=f"54324 1 {SIDECAR_BUNDLE_PATH}",
            args_output=(
                f"54324 {SIDECAR_BUNDLE_PATH} --port 11435 --data-dir {lookalike_dir}"
            ),
        ),
    )

    calls = []
    monkeypatch.setattr(os, "kill", _fake_kill_dies_immediately(calls))

    _reap_orphaned_sidecars(data_dir)

    assert calls == []


def test_reap_orphaned_by_args_leaves_non_orphan_alone(tmp_path, monkeypatch):
    data_dir = str(tmp_path / "Library" / "Application Support" / "com.ielegante.rumble")
    monkeypatch.setattr(
        subprocess,
        "run",
        _fake_ps_two_calls(
            comm_output=f"54323 4242 {SIDECAR_BUNDLE_PATH}",
            args_output=(
                f"54323 {SIDECAR_BUNDLE_PATH} --port 11435 --data-dir {data_dir}"
            ),
        ),
    )

    calls = []
    monkeypatch.setattr(os, "kill", _fake_kill_dies_immediately(calls))

    _reap_orphaned_sidecars(data_dir)

    assert calls == []


def test_reap_leaves_own_pid_alone(tmp_path, monkeypatch):
    # os.kill is stubbed rather than trusted to a real guard: if the pid == getpid()
    # check were ever removed or broken, letting this call through for real would
    # SIGTERM the test process itself.
    calls = []
    monkeypatch.setattr(os, "kill", lambda pid, sig: calls.append((pid, sig)))

    pidfile = tmp_path / "sidecar.pid"
    pidfile.write_text(str(os.getpid()))

    _reap_stray_sidecar(pidfile)

    assert calls == []


def test_watch_stdin_none_logs_and_returns(monkeypatch, caplog):
    monkeypatch.setattr(sys, "stdin", None)
    server = SimpleNamespace(should_exit=False)

    with caplog.at_level("WARNING"):
        _watch_stdin_for_eof(None, server)

    assert server.should_exit is False
    assert any("stdin" in rec.message.lower() for rec in caplog.records)
