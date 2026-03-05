# ABOUTME: PyInstaller spec for building the ivory-sidecar binary.
# ABOUTME: Bundles FastAPI + Uvicorn + PydanticAI into a one-dir distributable.

# -*- mode: python ; coding: utf-8 -*-

import sys
from pathlib import Path

block_cipher = None

a = Analysis(
    ["main.py"],
    pathex=[],
    binaries=[],
    datas=[],
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
    excludes=[],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="ivory-sidecar",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=True,
)

# One-dir mode (not one-file) to avoid PID confusion with Tauri
coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=True,
    upx_exclude=[],
    name="ivory-sidecar",
)
