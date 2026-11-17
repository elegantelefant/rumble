# ABOUTME: Runs scripts/smoke-macos-app.sh against synthetic .app bundles: it refuses library
# ABOUTME: validation disabled or the app without hardened runtime, and a helper that won't answer.

import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
SMOKE = REPO / "scripts" / "smoke-macos-app.sh"
APP_BINARY = "MacOS/elefant_rumble"
HELPER = "Helpers/rumble-sidecar.app/Contents/MacOS/rumble-sidecar"
DISABLE_LIBRARY_VALIDATION = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>com.apple.security.cs.disable-library-validation</key><true/>
</dict></plist>
"""

# Exits at once with PyInstaller's code for a Python library it couldn't load.
EXITS_255_C = "int main(void) { return 255; }\n"
# Hands its arguments to FAKE_SIDECAR_PY, like the real bootloader hands them to Python.
SERVES_HEALTH_C = """
#include <unistd.h>
int main(int argc, char **argv) {
    execv("/usr/bin/python3", (char *[]){"python3", SCRIPT, argv[1], argv[2], argv[3], argv[4], 0});
    return 127;
}
"""
# Answers /health only with the shared secret, as the sidecar does (app.py).
FAKE_SIDECAR_PY = """
import http.server, os, sys
port = int(sys.argv[sys.argv.index("--port") + 1])
secret = os.environ["RUMBLE_SIDECAR_SECRET"]
class Health(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        ok = self.path == "/health" and self.headers.get("X-Rumble-Secret") == secret
        self.send_response(200 if ok else 401)
        self.end_headers()
        self.wfile.write(b'{"status":"ok"}' if ok else b"")
    def log_message(self, *args):
        pass
http.server.HTTPServer(("127.0.0.1", port), Health).serve_forever()
"""

pytestmark = pytest.mark.skipif(sys.platform != "darwin", reason="codesign is macOS-only")

# How each executable is signed, as the bundler does for an ad-hoc build today: the app with the
# hardened runtime, the helper (PyInstaller's ad-hoc signature) without it.
SHIPPED = {APP_BINARY: {"runtime": True}, HELPER: {"runtime": False}}


def _build_app(tmp_path: Path, signing: dict[str, dict], serves_health: bool = False) -> Path:
    script = tmp_path / "fake_sidecar.py"
    script.write_text(FAKE_SIDECAR_PY)
    exits, serves = tmp_path / "exits.c", tmp_path / "serves.c"
    exits.write_text(EXITS_255_C)
    serves.write_text(SERVES_HEALTH_C)
    entitlements = tmp_path / "disable-library-validation.plist"
    entitlements.write_text(DISABLE_LIBRARY_VALIDATION)
    contents = tmp_path / "Rumble.app" / "Contents"
    for relative, how in signing.items():
        binary = contents / relative
        binary.parent.mkdir(parents=True, exist_ok=True)
        source = serves if serves_health and relative == HELPER else exits
        subprocess.run(["cc", f'-DSCRIPT="{script}"', str(source), "-o", str(binary)], check=True)
        sign = ["codesign", "--force", "-s", "-"]
        if how.get("runtime"):
            sign += ["--options", "runtime"]
        if how.get("disable_library_validation"):
            sign += ["--entitlements", str(entitlements)]
        subprocess.run([*sign, str(binary)], check=True, capture_output=True)
    return tmp_path / "Rumble.app"


def _smoke(app: Path) -> subprocess.CompletedProcess:
    return subprocess.run(["bash", str(SMOKE), str(app)], capture_output=True, text=True, check=False)


def test_the_smoke_passes_the_shipped_signing_when_the_helper_answers_health_with_the_secret(tmp_path):
    result = _smoke(_build_app(tmp_path, SHIPPED, serves_health=True))
    assert result.returncode == 0, result.stderr


def test_the_smoke_refuses_an_app_that_disables_library_validation(tmp_path):
    signing = {**SHIPPED, APP_BINARY: {"runtime": True, "disable_library_validation": True}}
    result = _smoke(_build_app(tmp_path, signing, serves_health=True))
    assert (result.returncode, "elefant_rumble disables library validation" in result.stderr) == (1, True)


def test_the_smoke_refuses_a_helper_that_disables_library_validation(tmp_path):
    signing = {**SHIPPED, HELPER: {"disable_library_validation": True}}
    result = _smoke(_build_app(tmp_path, signing, serves_health=True))
    assert (result.returncode, "rumble-sidecar disables library validation" in result.stderr) == (1, True)


def test_the_smoke_refuses_an_app_without_the_hardened_runtime(tmp_path):
    signing = {**SHIPPED, APP_BINARY: {"runtime": False}}
    result = _smoke(_build_app(tmp_path, signing, serves_health=True))
    assert (result.returncode, "elefant_rumble lacks the hardened runtime" in result.stderr) == (1, True)


def test_the_smoke_refuses_an_ad_hoc_helper_with_the_hardened_runtime(tmp_path):
    # Library validation would refuse its own libraries on a user's Mac; the CI runner doesn't enforce it.
    signing = {**SHIPPED, HELPER: {"runtime": True}}
    result = _smoke(_build_app(tmp_path, signing, serves_health=True))
    assert (result.returncode, "ad-hoc rumble-sidecar has the hardened runtime" in result.stderr) == (1, True)


def test_the_smoke_fails_when_the_helper_exits_before_answering(tmp_path):
    result = _smoke(_build_app(tmp_path, SHIPPED))
    assert (result.returncode, "sidecar exited with code 255 before /health answered" in result.stderr) == (1, True)
