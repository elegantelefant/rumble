# ABOUTME: Runs scripts/smoke-macos-app.sh against synthetic .app bundles to prove it refuses
# ABOUTME: an executable without the library-validation entitlement and a sidecar that won't start.

import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
SMOKE = REPO / "scripts" / "smoke-macos-app.sh"
ENTITLEMENTS = REPO / "src-tauri" / "Entitlements.plist"
APP_BINARY = "elefant_rumble"
SIDECAR = "rumble-sidecar"

# Exits at once with PyInstaller's code for a Python library it couldn't load.
EXITS_255_C = "int main(void) { return 255; }\n"

pytestmark = pytest.mark.skipif(sys.platform != "darwin", reason="codesign is macOS-only")


def _build_app(tmp_path: Path, entitled: dict[str, bool]) -> Path:
    source = tmp_path / "exits.c"
    source.write_text(EXITS_255_C)
    macos = tmp_path / "Rumble.app" / "Contents" / "MacOS"
    macos.mkdir(parents=True)
    for name, with_entitlements in entitled.items():
        binary = macos / name
        subprocess.run(["cc", str(source), "-o", str(binary)], check=True)
        sign = ["codesign", "--force", "-s", "-", "--options", "runtime"]
        if with_entitlements:
            sign += ["--entitlements", str(ENTITLEMENTS)]
        subprocess.run([*sign, str(binary)], check=True, capture_output=True)
    return tmp_path / "Rumble.app"


def _smoke(app: Path) -> subprocess.CompletedProcess:
    return subprocess.run(["bash", str(SMOKE), str(app)], capture_output=True, text=True, check=False)


def test_the_smoke_refuses_a_sidecar_signed_without_the_entitlement(tmp_path):
    result = _smoke(_build_app(tmp_path, {APP_BINARY: True, SIDECAR: False}))
    assert (result.returncode, f"{SIDECAR} lacks com.apple.security.cs.disable-library-validation" in result.stderr) == (1, True)


def test_the_smoke_refuses_an_app_binary_signed_without_the_entitlement(tmp_path):
    result = _smoke(_build_app(tmp_path, {APP_BINARY: False, SIDECAR: True}))
    assert (result.returncode, f"{APP_BINARY} lacks com.apple.security.cs.disable-library-validation" in result.stderr) == (1, True)


def test_the_smoke_fails_when_the_bundled_sidecar_exits_before_answering(tmp_path):
    result = _smoke(_build_app(tmp_path, {APP_BINARY: True, SIDECAR: True}))
    assert (result.returncode, "sidecar exited with code 255 before /health answered" in result.stderr) == (1, True)
