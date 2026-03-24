# ABOUTME: FastAPI application factory for the rumble sidecar.
# ABOUTME: Wires up lifespan (DB init/close) and route modules.
# TODO: Add authentication middleware (bearer token or shared secret from Tauri host)

from contextlib import asynccontextmanager

from fastapi import FastAPI

from services.db import close_db, init_db
from services.jobs import shutdown as shutdown_jobs


@asynccontextmanager
async def lifespan(app: FastAPI):
    await init_db(app.state.data_dir)
    yield
    await shutdown_jobs()
    await close_db()


def create_app(data_dir: str | None = None) -> FastAPI:
    app = FastAPI(title="rumble-sidecar", version="0.1.0", lifespan=lifespan)
    app.state.data_dir = data_dir

    from routes.ai import router as ai_router
    from routes.chat import router as chat_router
    from routes.health import router as health_router
    from routes.jobs import router as jobs_router

    app.include_router(health_router)
    app.include_router(chat_router)
    app.include_router(ai_router)
    app.include_router(jobs_router)
    return app
