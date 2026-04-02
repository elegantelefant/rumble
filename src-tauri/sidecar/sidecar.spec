# ABOUTME: PyInstaller spec for building the rumble-sidecar binary.
# ABOUTME: Bundles FastAPI + Uvicorn + PydanticAI into a one-dir distributable.

# -*- mode: python ; coding: utf-8 -*-

import sys
from pathlib import Path

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
        "services",
        "services.db",
        "services.llm",
        "services.jobs",
        "services.prompts",
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

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name="rumble-sidecar",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=True,
)
