# ABOUTME: Runs scripts/smoke-macos-single-instance.sh against fake .app bundles to prove it fails
# ABOUTME: when a second launch keeps running, or the first never stays up, and passes otherwise.

import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SMOKE = REPO / "scripts" / "smoke-macos-single-instance.sh"
FAST = {"RUMBLE_SMOKE_SETTLE_SECONDS": "1", "RUMBLE_SMOKE_EXIT_WAIT_SECONDS": "2"}

# Stays up unless a "first" marker exists, which stands in for the instance lock.
SINGLE_INSTANCE = """#!/bin/bash
lock="$(dirname "$0")/lock"
if [ -e "$lock" ]; then exit 0; fi
touch "$lock"
trap 'rm -f "$lock"' EXIT
sleep 30 & wait
"""


def _app(tmp_path: Path, body: str) -> Path:
    macos = tmp_path / "Rumble.app" / "Contents" / "MacOS"
    macos.mkdir(parents=True)
    binary = macos / "elefant_rumble"
    binary.write_text(body)
    binary.chmod(0o755)
    return tmp_path / "Rumble.app"


def _smoke(app: Path) -> subprocess.CompletedProcess:
    import os

    return subprocess.run(
        ["bash", str(SMOKE), str(app)], capture_output=True, text=True, check=False, env={**os.environ, **FAST}
    )


def test_the_smoke_fails_when_a_second_launch_keeps_running(tmp_path):
    result = _smoke(_app(tmp_path, "#!/bin/bash\nsleep 30 & wait\n"))
    assert (result.returncode, "second instance was still running" in result.stderr) == (1, True)


def test_the_smoke_fails_when_the_first_launch_does_not_stay_up(tmp_path):
    result = _smoke(_app(tmp_path, "#!/bin/bash\nexit 3\n"))
    assert (result.returncode, "first instance exited" in result.stderr) == (1, True)


def test_the_smoke_passes_when_a_second_launch_exits_and_the_first_stays(tmp_path):
    result = _smoke(_app(tmp_path, SINGLE_INSTANCE))
    assert result.returncode == 0, result.stderr
