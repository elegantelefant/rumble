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


def cli(argv: list[str] | None = None):
    parser = argparse.ArgumentParser(description="Rumble sidecar server")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    parser.add_argument("--data-dir", type=str, default=None, help="Directory for SQLite DB")
    parser.add_argument("--secret", type=str, default=None, help="Shared secret required on all requests (supplied by Tauri host)")
    parser.add_argument(
        "--dev",
        action="store_true",
        help="Run without a shared secret, unauthenticated. Development only — never use outside local development.",
    )
    args = parser.parse_args(argv)

    secret = resolve_secret(args.secret)
    if not secret and not args.dev:
        print(
            "error: no shared secret configured (set RUMBLE_SIDECAR_SECRET, pass --secret, "
            "or pass --dev for local development)",
            file=sys.stderr,
        )
        sys.exit(1)

    # Set by the Tauri host at spawn only; --dev runs and pytest never set it, so a
    # closed/redirected stdin there can't trigger a shutdown.
    watch_stdin = os.environ.get("RUMBLE_SIDECAR_WATCH_STDIN") == "1"
    app = create_app(data_dir=args.data_dir, secret=secret, dev=args.dev, watch_stdin=watch_stdin)

    config = uvicorn.Config(app, host="127.0.0.1", port=args.port, log_level="warning")
    server = uvicorn.Server(config)
    app.state.server = server

    original_startup = server.startup

    async def _startup_with_signal(*a, **kw):
        await original_startup(*a, **kw)
        # Signal port AFTER socket is bound and server is accepting connections
        print(f"PORT:{args.port}", flush=True)

    server.startup = _startup_with_signal
    server.run()


if __name__ == "__main__":
    cli()
