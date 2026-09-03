# ABOUTME: CLI entry point for the rumble sidecar server.
# ABOUTME: Parses --port and --data-dir args, prints PORT:{port} for Rust, runs uvicorn.

import argparse
import os
import sys

import uvicorn

from app import create_app

DEFAULT_PORT = 11435


class _StartupPrinter(uvicorn.config.Config):
    """Defers PORT announcement until uvicorn has bound the socket."""


def resolve_secret(flag_value: str | None) -> str | None:
    """Resolve the shared secret, preferring the environment over argv.

    The Tauri host passes the secret via RUMBLE_SIDECAR_SECRET because
    command-line arguments are visible to any process via `ps`. The --secret
    flag remains for manual dev runs.
    """
    return os.environ.get("RUMBLE_SIDECAR_SECRET") or flag_value


def cli():
    parser = argparse.ArgumentParser(description="Rumble sidecar server")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    parser.add_argument("--data-dir", type=str, default=None, help="Directory for SQLite DB")
    parser.add_argument("--secret", type=str, default=None, help="Shared secret required on all requests (supplied by Tauri host)")
    args = parser.parse_args()

    app = create_app(data_dir=args.data_dir, secret=resolve_secret(args.secret))

    config = uvicorn.Config(app, host="127.0.0.1", port=args.port, log_level="warning")
    server = uvicorn.Server(config)

    original_startup = server.startup

    async def _startup_with_signal(*a, **kw):
        await original_startup(*a, **kw)
        # Signal port AFTER socket is bound and server is accepting connections
        print(f"PORT:{args.port}", flush=True)

    server.startup = _startup_with_signal
    server.run()


if __name__ == "__main__":
    cli()
