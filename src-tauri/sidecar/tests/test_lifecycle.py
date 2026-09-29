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

import app as app_module
from app import (
    STDIN_WATCHER_THREAD_NAME,
    _reap_strays,
    _terminate,
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

    application = create_app(data_dir=str(tmp_path), dev=True, watch_stdin=False)
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

    application = create_app(data_dir=str(tmp_path), dev=True, watch_stdin=False)
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

        _reap_strays(str(tmp_path), pidfile)

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

    _reap_strays(str(tmp_path), pidfile)  # no-op on a dead PID, must not raise
    _write_pidfile(pidfile)

    assert pidfile.read_text().strip() == str(os.getpid())


# The real installed path — has spaces in it, which is exactly what a
# first-word-of-args split would get wrong.
SIDECAR_BUNDLE_PATH = "/Applications/Elefant - Rumble.app/Contents/MacOS/rumble-sidecar"
HOST_BUNDLE_PATH = "/Applications/Elefant - Rumble.app/Contents/MacOS/elefant_rumble"
STRAY_PID = 54321
BOOTLOADER_PID = 54320
HOST_PID = 4242
SUBREAPER_PID = 900


def _data_dir(tmp_path, bundle_id="com.ielegante.rumble"):
    return str(tmp_path / "Library" / "Application Support" / bundle_id)


def _sidecar(pid, ppid, data_dir):
    return (pid, ppid, SIDECAR_BUNDLE_PATH, f"{SIDECAR_BUNDLE_PATH} --port 11435 --data-dir {data_dir}")


def _fake_ps(processes):
    """_process_table makes two separate `ps` calls (one for pid/ppid/comm, one
    for pid/args) — dispatch on which columns were asked for rather than call
    order, so the fake doesn't depend on that detail."""
    comm_output = "\n".join(f"{pid} {ppid} {comm}" for pid, ppid, comm, _ in processes)
    args_output = "\n".join(f"{pid} {args}" for pid, _, _, args in processes)

    def _run(cmd, **kwargs):
        o_value = cmd[cmd.index("-o") + 1]
        return SimpleNamespace(stdout=comm_output if "comm=" in o_value else args_output)

    return _run


def _fake_kill_dies_immediately(calls):
    """Records every os.kill call; a SIGTERM succeeds, and the liveness probe
    that follows reports the process already gone, so _terminate's poll loop
    exits on its first check instead of really sleeping."""

    def _fake_kill(pid, sig):
        calls.append((pid, sig))
        if sig != 0:
            return
        raise ProcessLookupError

    return _fake_kill


def _reap_with(tmp_path, monkeypatch, processes, data_dir, pidfile_pid=None):
    monkeypatch.setattr(subprocess, "run", _fake_ps(processes))
    calls = []
    monkeypatch.setattr(os, "kill", _fake_kill_dies_immediately(calls))
    pidfile = tmp_path / "sidecar.pid"
    if pidfile_pid is not None:
        pidfile.write_text(str(pidfile_pid))
    _reap_strays(data_dir, pidfile)
    return calls


def test_reap_kills_sidecar_orphaned_to_init(tmp_path, monkeypatch):
    data_dir = _data_dir(tmp_path)
    calls = _reap_with(tmp_path, monkeypatch, [_sidecar(STRAY_PID, 1, data_dir)], data_dir)

    assert (STRAY_PID, signal.SIGTERM) in calls


def test_reap_kills_sidecar_orphaned_to_systemd_user_subreaper(tmp_path, monkeypatch):
    data_dir = _data_dir(tmp_path)
    processes = [
        (SUBREAPER_PID, 1, "systemd", "/usr/lib/systemd/systemd --user"),
        _sidecar(STRAY_PID, SUBREAPER_PID, data_dir),
    ]
    calls = _reap_with(tmp_path, monkeypatch, processes, data_dir)

    assert (STRAY_PID, signal.SIGTERM) in calls


def test_reap_kills_child_of_orphaned_bootloader(tmp_path, monkeypatch):
    data_dir = _data_dir(tmp_path)
    processes = [_sidecar(BOOTLOADER_PID, 1, data_dir), _sidecar(STRAY_PID, BOOTLOADER_PID, data_dir)]
    calls = _reap_with(tmp_path, monkeypatch, processes, data_dir)

    assert (STRAY_PID, signal.SIGTERM) in calls


def _live_sibling(data_dir):
    """Another instance (e.g. `pnpm tauri dev` next to the packaged app) serving
    the same data dir: host -> bootloader -> Python child, all alive."""
    return [
        (HOST_PID, 1, HOST_BUNDLE_PATH, HOST_BUNDLE_PATH),
        _sidecar(BOOTLOADER_PID, HOST_PID, data_dir),
        _sidecar(STRAY_PID, BOOTLOADER_PID, data_dir),
    ]


def test_reap_leaves_live_sibling_instance_alone(tmp_path, monkeypatch):
    data_dir = _data_dir(tmp_path)
    calls = _reap_with(tmp_path, monkeypatch, _live_sibling(data_dir), data_dir, pidfile_pid=STRAY_PID)

    assert calls == []


def test_reap_logs_why_it_skipped_a_sidecar(tmp_path, monkeypatch, caplog):
    data_dir = _data_dir(tmp_path)
    with caplog.at_level("WARNING"):
        _reap_with(tmp_path, monkeypatch, _live_sibling(data_dir), data_dir)

    assert any(f"leaving sidecar pid {STRAY_PID} alone" in r.message and "alive" in r.message for r in caplog.records)


def test_reap_leaves_different_data_dir_alone(tmp_path, monkeypatch):
    data_dir = _data_dir(tmp_path)
    other = _sidecar(STRAY_PID, 1, _data_dir(tmp_path, "com.other.app"))
    calls = _reap_with(tmp_path, monkeypatch, [other], data_dir, pidfile_pid=STRAY_PID)

    assert calls == []


def test_reap_leaves_prefix_sharing_data_dir_alone(tmp_path, monkeypatch):
    # "com.ielegante.rumble-dev" starts with "com.ielegante.rumble" — a plain
    # substring check on the needle would wrongly match this.
    data_dir = _data_dir(tmp_path)
    lookalike = _sidecar(STRAY_PID, 1, _data_dir(tmp_path, "com.ielegante.rumble-dev"))
    calls = _reap_with(tmp_path, monkeypatch, [lookalike], data_dir)

    assert calls == []


def test_reap_sends_no_signal_to_a_non_sidecar_pidfile_pid(tmp_path, monkeypatch):
    # Not even the os.kill(pid, 0) probe: on Windows that is CTRL_C_EVENT. The
    # args match on purpose (a `pnpm dev:sidecar` run), so only the name differs.
    data_dir = _data_dir(tmp_path)
    reused = (STRAY_PID, 1, "/usr/bin/python3", f"/usr/bin/python3 main.py --data-dir {data_dir}")
    calls = _reap_with(tmp_path, monkeypatch, [reused], data_dir, pidfile_pid=STRAY_PID)

    assert calls == []


def test_reap_leaves_own_pid_alone(tmp_path, monkeypatch):
    # os.kill is stubbed rather than trusted to a real guard: if the pid == getpid()
    # check were ever removed or broken, letting this call through for real would
    # SIGTERM the test process itself.
    # The table lists this process as an orphaned sidecar, as a frozen one whose
    # bootloader was SIGKILLed would be, so only the own-PID guard stands in the way.
    data_dir = _data_dir(tmp_path)
    own = _sidecar(os.getpid(), 1, data_dir)
    calls = _reap_with(tmp_path, monkeypatch, [own], data_dir, pidfile_pid=os.getpid())

    assert calls == []


def test_reap_on_windows_touches_nothing_and_says_so(tmp_path, monkeypatch, caplog):
    monkeypatch.setattr(sys, "platform", "win32")
    data_dir = _data_dir(tmp_path)
    with caplog.at_level("WARNING"):
        calls = _reap_with(tmp_path, monkeypatch, [_sidecar(STRAY_PID, 1, data_dir)], data_dir)

    assert calls == [] and any("Windows" in r.message for r in caplog.records)


def test_terminate_escalates_to_sigkill_when_sigterm_is_ignored(monkeypatch):
    monkeypatch.setattr(app_module, "REAP_TERM_WAIT_SECONDS", 0.2)
    proc = subprocess.Popen(
        [
            sys.executable,
            "-c",
            "import signal, time; signal.signal(signal.SIGTERM, signal.SIG_IGN); print('ready', flush=True); time.sleep(30)",
        ],
        stdout=subprocess.PIPE,
    )
    try:
        proc.stdout.readline()
        # Collect the exit status as soon as it happens, or the zombie would
        # still answer the liveness probe.
        threading.Thread(target=proc.wait, daemon=True).start()

        _terminate(proc.pid)

        assert proc.wait(timeout=5) == -signal.SIGKILL
    finally:
        proc.kill()
        proc.stdout.close()


def test_terminate_logs_error_when_stray_survives_sigkill(monkeypatch, caplog):
    monkeypatch.setattr(app_module, "REAP_TERM_WAIT_SECONDS", 0.01)
    monkeypatch.setattr(app_module, "REAP_KILL_WAIT_SECONDS", 0.01)
    monkeypatch.setattr(os, "kill", lambda pid, sig: None)

    with caplog.at_level("ERROR"):
        _terminate(STRAY_PID)

    assert any(r.levelname == "ERROR" and "survived SIGKILL" in r.message for r in caplog.records)


def test_watch_stdin_none_logs_and_returns(monkeypatch, caplog):
    monkeypatch.setattr(sys, "stdin", None)
    server = SimpleNamespace(should_exit=False)

    with caplog.at_level("WARNING"):
        _watch_stdin_for_eof(None, server)

    assert server.should_exit is False
    assert any("stdin" in rec.message.lower() for rec in caplog.records)
