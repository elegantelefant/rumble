# ABOUTME: PyInstaller spec for building the rumble-sidecar binary.
# ABOUTME: Bundles FastAPI + Uvicorn + PydanticAI: onedir on macOS (an app helper), onefile elsewhere.

# -*- mode: python ; coding: utf-8 -*-

import sys

from PyInstaller.utils.hooks import copy_metadata

block_cipher = None

# Several packages use importlib.metadata.version() at import time.
_meta_pkgs = [
    "genai_prices", "pydantic_ai", "pydantic_ai_slim",
    "pydantic", "httpx", "fastapi", "starlette", "uvicorn",
    "sse_starlette", "aiosqlite",
]
datas = [item for pkg in _meta_pkgs for item in copy_metadata(pkg)]

a = Analysis(
    ["main.py"],
    pathex=[],
    binaries=[],
    datas=datas,
    hiddenimports=[
        # Uvicorn internals not auto-detected
        "uvicorn.lifespan.on",
        "uvicorn.lifespan.off",
        "uvicorn.logging",
        "uvicorn.loops.auto",
        "uvicorn.loops.asyncio",
        "uvicorn.protocols.http.auto",
        "uvicorn.protocols.http.h11_impl",
        "uvicorn.protocols.http.httptools_impl",
        "uvicorn.protocols.websockets.auto",
        "uvicorn.protocols.websockets.wsproto_impl",
        # FastAPI/Starlette internals
        "python_multipart",
        "python_multipart.multipart",
        "multipart",
        "multipart.multipart",
        # PydanticAI
        "pydantic_ai",
        "pydantic_ai.models.openai",
        # App modules
        "app",
        "routes",
        "routes.health",
        "routes.chat",
        "routes.ai",
        "routes.jobs",
        "routes.extract",
        "services",
        "services.db",
        "services.llm",
        "services.jobs",
        "services.prompts",
        "services.extract",
        "models",
        "models.generated",
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        # logfire hooks into Pydantic and calls inspect.getsource() which fails in frozen binaries
        "logfire",
        "logfire.integrations",
    ],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

# macOS: onedir wrapped in its own app bundle (code in MacOS/Frameworks, data in Resources, as
# Apple's bundle layout requires), shipped as a helper inside the signed .app (Contents/Helpers):
# nothing is unpacked to a temp folder at launch, and every library is sealed by a signature (#157).
# Elsewhere: onefile, shipped as a Tauri externalBin.
ONEDIR = sys.platform == "darwin"

exe = EXE(
    pyz,
    a.scripts,
    *([] if ONEDIR else [a.binaries, a.zipfiles, a.datas]),
    [],
    exclude_binaries=ONEDIR,
    name="rumble-sidecar",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=True,
)

if ONEDIR:
    coll = COLLECT(exe, a.binaries, a.zipfiles, a.datas, strip=False, upx=False, name="rumble-sidecar")
    app = BUNDLE(
        coll,
        name="rumble-sidecar.app",
        bundle_identifier="legal.elefant.rumble.sidecar",
        # A background helper: no Dock icon, no menu bar.
        info_plist={"LSBackgroundOnly": True},
    )
