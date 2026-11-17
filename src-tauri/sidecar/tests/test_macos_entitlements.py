# ABOUTME: Proves the macOS entitlements Tauri signs every bundled executable with let a
# ABOUTME: hardened-runtime, ad-hoc-signed executable load an ad-hoc-signed library (#157).

import json
import subprocess
import sys
from pathlib import Path

import pytest

SRC_TAURI = Path(__file__).resolve().parents[2]
TAURI_CONFIG = SRC_TAURI / "tauri.conf.json"

LIBRARY_C = "int answer(void) { return 42; }\n"
# Stands in for PyInstaller's bootloader dlopen-ing the Python library it unpacked.
LOADER_C = """
#include <dlfcn.h>
#include <stdio.h>
int main(int argc, char **argv) {
    void *lib = dlopen(argv[1], RTLD_NOW);
    if (!lib) { fprintf(stderr, "%s\\n", dlerror()); return 1; }
    return 0;
}
"""


def entitlements_file() -> Path:
    config = json.loads(TAURI_CONFIG.read_text())
    return SRC_TAURI / config["bundle"]["macOS"]["entitlements"]


def test_tauri_signs_with_the_repo_entitlements_file():
    assert entitlements_file().is_file()


def _run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(args, capture_output=True, text=True, check=False)


@pytest.mark.skipif(sys.platform != "darwin", reason="library validation is macOS-only")
def test_a_hardened_adhoc_executable_loads_an_adhoc_library(tmp_path):
    (tmp_path / "answer.c").write_text(LIBRARY_C)
    (tmp_path / "loader.c").write_text(LOADER_C)
    library, loader = tmp_path / "libanswer.dylib", tmp_path / "loader"
    for build in (
        ["cc", "-dynamiclib", str(tmp_path / "answer.c"), "-o", str(library)],
        ["cc", str(tmp_path / "loader.c"), "-o", str(loader)],
        ["codesign", "--force", "-s", "-", str(library)],
        # What the Tauri bundler does to the sidecar with APPLE_SIGNING_IDENTITY=- and
        # hardenedRuntime: the ad-hoc identity, the runtime option, the entitlements file.
        ["codesign", "--force", "-s", "-", "--options", "runtime", "--entitlements", str(entitlements_file()), str(loader)],
    ):
        assert _run(*build).returncode == 0, build

    result = _run(str(loader), str(library))

    assert result.returncode == 0, result.stderr
