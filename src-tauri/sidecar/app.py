# ABOUTME: FastAPI application factory for the rumble sidecar.
# ABOUTME: Wires up lifespan (DB init/close), shared-secret auth, and route modules.
import logging
import secrets
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse

from services.db import close_db, init_db
from services.jobs import shutdown as shutdown_jobs

SECRET_HEADER = "x-rumble-secret"

logger = logging.getLogger(__name__)


@asynccontextmanager
async def lifespan(app: FastAPI):
    await init_db(app.state.data_dir)
    yield
    await shutdown_jobs()
    await close_db()


def create_app(data_dir: str | None = None, secret: str | None = None, dev: bool = False) -> FastAPI:
    if not secret and not dev:
        raise ValueError("create_app requires either a secret or dev=True")

    app = FastAPI(title="rumble-sidecar", version="0.1.0", lifespan=lifespan)
    app.state.data_dir = data_dir
    app.state.secret = secret

    if secret:

        @app.middleware("http")
        async def verify_shared_secret(request: Request, call_next):
            provided = request.headers.get(SECRET_HEADER) or ""
            if not secrets.compare_digest(provided, app.state.secret):
                return JSONResponse(
                    status_code=401,
                    content={"detail": "missing or invalid shared secret"},
                )
            return await call_next(request)
    else:
        logger.warning(
            "Sidecar running with --dev and no shared secret: every endpoint is UNAUTHENTICATED. "
            "Never use --dev outside local development."
        )

    from routes.ai import router as ai_router
    from routes.chat import router as chat_router
    from routes.extract import router as extract_router
    from routes.health import router as health_router
    from routes.jobs import router as jobs_router

    app.include_router(health_router)
    app.include_router(chat_router)
    app.include_router(ai_router)
    app.include_router(jobs_router)
    app.include_router(extract_router)
    return app
