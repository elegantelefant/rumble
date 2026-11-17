# ABOUTME: Runs scripts/smoke-macos-app.sh against synthetic .app bundles: it refuses an executable
# ABOUTME: without the library-validation entitlement (or with it false) and a sidecar that won't answer.

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
# Hands its arguments to FAKE_SIDECAR_PY, like the real bootloader hands them to Python.
SERVES_HEALTH_C = """
#include <unistd.h>
int main(int argc, char **argv) {
    argv[0] = SCRIPT;
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


def _build_app(tmp_path: Path, entitled: dict[str, Path | None], serves_health: bool = False) -> Path:
    """`entitled` maps each executable to the entitlements file it is signed with (None: none)."""
    script = tmp_path / "fake_sidecar.py"
    script.write_text(FAKE_SIDECAR_PY)
    exits, serves = tmp_path / "exits.c", tmp_path / "serves.c"
    exits.write_text(EXITS_255_C)
    serves.write_text(SERVES_HEALTH_C)
    macos = tmp_path / "Rumble.app" / "Contents" / "MacOS"
    macos.mkdir(parents=True)
    for name, entitlements in entitled.items():
        binary = macos / name
        source = serves if serves_health and name == SIDECAR else exits
        subprocess.run(["cc", f'-DSCRIPT="{script}"', str(source), "-o", str(binary)], check=True)
        sign = ["codesign", "--force", "-s", "-", "--options", "runtime"]
        if entitlements:
            sign += ["--entitlements", str(entitlements)]
        subprocess.run([*sign, str(binary)], check=True, capture_output=True)
    return tmp_path / "Rumble.app"


def _entitlement_set_false(tmp_path: Path) -> Path:
    plist = tmp_path / "false.plist"
    plist.write_text(ENTITLEMENTS.read_text().replace("<true/>", "<false/>"))
    return plist


def _smoke(app: Path) -> subprocess.CompletedProcess:
    return subprocess.run(["bash", str(SMOKE), str(app)], capture_output=True, text=True, check=False)


def test_the_smoke_refuses_a_sidecar_signed_without_the_entitlement(tmp_path):
    result = _smoke(_build_app(tmp_path, {APP_BINARY: ENTITLEMENTS, SIDECAR: None}))
    assert (result.returncode, f"{SIDECAR} lacks com.apple.security.cs.disable-library-validation" in result.stderr) == (1, True)


def test_the_smoke_refuses_an_app_binary_signed_without_the_entitlement(tmp_path):
    result = _smoke(_build_app(tmp_path, {APP_BINARY: None, SIDECAR: ENTITLEMENTS}))
    assert (result.returncode, f"{APP_BINARY} lacks com.apple.security.cs.disable-library-validation" in result.stderr) == (1, True)


def test_the_smoke_refuses_the_entitlement_set_to_false(tmp_path):
    result = _smoke(_build_app(tmp_path, {APP_BINARY: ENTITLEMENTS, SIDECAR: _entitlement_set_false(tmp_path)}))
    assert (result.returncode, f"{SIDECAR} lacks com.apple.security.cs.disable-library-validation" in result.stderr) == (1, True)


def test_the_smoke_passes_when_both_are_entitled_and_the_sidecar_answers_health_with_the_secret(tmp_path):
    result = _smoke(_build_app(tmp_path, {APP_BINARY: ENTITLEMENTS, SIDECAR: ENTITLEMENTS}, serves_health=True))
    assert result.returncode == 0, result.stderr


def test_the_smoke_fails_when_the_bundled_sidecar_exits_before_answering(tmp_path):
    result = _smoke(_build_app(tmp_path, {APP_BINARY: ENTITLEMENTS, SIDECAR: ENTITLEMENTS}))
    assert (result.returncode, "sidecar exited with code 255 before /health answered" in result.stderr) == (1, True)
