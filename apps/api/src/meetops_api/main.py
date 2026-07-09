"""FastAPI application factory."""

from __future__ import annotations

import logging
from contextlib import asynccontextmanager

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from slowapi import _rate_limit_exceeded_handler
from slowapi.errors import RateLimitExceeded
from slowapi.middleware import SlowAPIMiddleware
from starlette.exceptions import HTTPException as StarletteHTTPException

from meetops_api.errors import (
    AppError,
    app_error_handler,
    http_exception_handler,
    unhandled_exception_handler,
)
from meetops_api.logging import setup_logging
from meetops_api.rate_limit import limiter
from meetops_api.routers import auth, bot, calendars, meetings, notes, recordings
from meetops_api.routers import settings as settings_router
from meetops_common.db import dispose_engine, get_engine
from meetops_common.settings import get_settings

logger = logging.getLogger(__name__)


@asynccontextmanager
async def lifespan(app: FastAPI):
    cfg = get_settings()
    setup_logging(cfg)
    get_engine(cfg)
    cfg.meetops_data_dir.mkdir(parents=True, exist_ok=True)
    logger.info("MeetOps API starting env=%s", cfg.meetops_env)
    yield
    await dispose_engine()
    logger.info("MeetOps API stopped")


def create_app() -> FastAPI:
    cfg = get_settings()
    app = FastAPI(
        title="MeetOps API",
        version="0.1.0",
        docs_url="/docs" if cfg.meetops_enable_openapi else None,
        redoc_url="/redoc" if cfg.meetops_enable_openapi else None,
        lifespan=lifespan,
    )
    app.state.limiter = limiter
    app.add_exception_handler(RateLimitExceeded, _rate_limit_exceeded_handler)
    app.add_exception_handler(AppError, app_error_handler)
    app.add_exception_handler(StarletteHTTPException, http_exception_handler)
    app.add_exception_handler(Exception, unhandled_exception_handler)
    app.add_middleware(SlowAPIMiddleware)
    app.add_middleware(
        CORSMiddleware,
        allow_origins=cfg.cors_origin_list,
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )

    @app.get("/healthz")
    async def healthz() -> dict[str, str]:
        return {"status": "ok"}

    app.include_router(auth.router, prefix="/api/v1")
    app.include_router(settings_router.router, prefix="/api/v1")
    app.include_router(calendars.router, prefix="/api/v1")
    app.include_router(meetings.router, prefix="/api/v1")
    app.include_router(recordings.router, prefix="/api/v1")
    app.include_router(notes.router_notes, prefix="/api/v1")
    app.include_router(notes.router_todos, prefix="/api/v1")
    app.include_router(bot.router, prefix="/api/v1")
    app.include_router(calendars.oauth_router)

    return app


def run() -> None:
    import uvicorn

    cfg = get_settings()
    setup_logging(cfg)
    uvicorn.run(
        "meetops_api.main:create_app",
        factory=True,
        host=cfg.meetops_api_host,
        port=cfg.meetops_api_port,
        log_level=cfg.meetops_log_level.lower(),
    )


app = create_app()