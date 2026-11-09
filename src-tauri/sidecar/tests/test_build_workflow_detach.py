# ABOUTME: Pins macOS DMG detach retries and harmless cleanup in the build workflow.
# ABOUTME: Exercises the workflow's Bash helper with busy and successful mounts.

import re
import subprocess
import textwrap
from pathlib import Path

import pytest

WORKFLOW = Path(__file__).resolve().parents[3] / ".github/workflows/build.yml"


def verify_script():
    workflow = WORKFLOW.read_text()
    step = workflow.split(
        "      - name: Verify macOS app and launch bundled sidecar\n", 1
    )[1].split("\n      - name:", 1)[0]
    return textwrap.dedent(step.split("        run: |\n", 1)[1])


@pytest.mark.parametrize("success_on", [0, 1, 3, 5])
def test_detach_mount_retries_and_forces_last_attempt(success_on):
    script = verify_script()
    helper = re.search(r"^detach_mount\(\) \{\n.*?^\}", script, re.MULTILINE | re.DOTALL)
    assert helper is not None
    assert "for attempt in 1 2 3 4 5; do" in helper[0]
    assert 'if [ "$attempt" -eq 5 ]; then' in helper[0]
    assert 'hdiutil detach -force "$mount_dir"' in helper[0]
    result = subprocess.run(
        ["bash", "-c", textwrap.dedent("""\
            set -euo pipefail
            mount_dir='/tmp/mount with spaces'
            calls=0
            hdiutil() {
              calls=$((calls + 1))
              printf '%s\\n' "$*"
              [ "$calls" -eq "$success_on" ]
            }
            sleep() { printf 'sleep %s\\n' "$*"; }
            """) + f"success_on={success_on}\n" + helper[0] + "\ndetach_mount\n"],
        capture_output=True,
        text=True,
        check=True,
    )
    attempts = success_on or 5
    expected = []
    for attempt in range(1, attempts + 1):
        force = "-force " if attempt == 5 else ""
        expected.append(f"detach {force}/tmp/mount with spaces")
        if attempt < attempts:
            expected.append("sleep 2")
    assert result.stdout.splitlines() == expected


def test_verify_loop_and_exit_trap_use_safe_cleanup():
    script = verify_script()
    assert (
        "trap 'detach_mount >/dev/null 2>&1 || true; "
        'rmdir "$mount_dir" || true\' EXIT'
    ) in script
    loop = script.split('for dmg in "${dmgs[@]}"; do\n', 1)[1]
    assert loop.splitlines()[-2:] == ["  detach_mount", "done"]
    assert '  codesign --verify --deep --strict --verbose=2 "$app"\n' in loop
    assert '  "$app/Contents/MacOS/rumble-sidecar" --help\n' in loop
    assert script.startswith("set -euo pipefail\n")
