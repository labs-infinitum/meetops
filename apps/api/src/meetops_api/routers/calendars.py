"""Calendar routes + OAuth callback."""

from __future__ import annotations

import logging

from fastapi import APIRouter, BackgroundTasks, Query
from fastapi.responses import RedirectResponse
from sqlalchemy import select

from meetops_api.crypto.tokens import TokenCrypto
from meetops_api.deps import AppSettings, Crypto, CurrentUser, DbSession
from meetops_api.errors import AppError
from meetops_api.google.calendar import CalendarSync
from meetops_api.google.oauth import GoogleOAuth
from meetops_api.schemas import (
    CalendarConnectionOut,
    OAuthStartRequest,
    OAuthStartResponse,
    SyncResult,
)
from meetops_common.models import CalendarConnection

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/calendars", tags=["calendars"])
oauth_router = APIRouter(tags=["oauth"])


@router.get("/", response_model=list[CalendarConnectionOut])
async def list_connections(user: CurrentUser, db: DbSession) -> list[CalendarConnectionOut]:
    rows = await db.scalars(
        select(CalendarConnection)
        .where(CalendarConnection.user_id == user.id)
        .order_by(CalendarConnection.created_at.desc())
    )
    return [CalendarConnectionOut.model_validate(r) for r in rows]


@router.post("/oauth/start", response_model=OAuthStartResponse)
async def oauth_start(
    body: OAuthStartRequest,
    user: CurrentUser,
    db: DbSession,
    settings: AppSettings,
    crypto: Crypto,
) -> OAuthStartResponse:
    if body.provider != "google":
        raise AppError("unsupported_provider", "Only Google Calendar is supported in v1", 400)
    oauth = GoogleOAuth(db, settings, crypto)
    url = await oauth.start(user.id, label=body.label, color=body.color)
    return OAuthStartResponse(redirect_url=url)


@router.delete("/{connection_id}")
async def disconnect(connection_id: str, user: CurrentUser, db: DbSession) -> dict[str, str]:
    conn = await db.get(CalendarConnection, connection_id)
    if conn is None or conn.user_id != user.id:
        raise AppError("not_found", "Calendar connection not found", 404)
    await db.delete(conn)
    return {"message": "Disconnected"}


@router.post("/{connection_id}/sync", response_model=SyncResult)
async def sync_now(
    connection_id: str,
    user: CurrentUser,
    db: DbSession,
    settings: AppSettings,
    crypto: Crypto,
) -> SyncResult:
    conn = await db.get(CalendarConnection, connection_id)
    if conn is None or conn.user_id != user.id:
        raise AppError("not_found", "Calendar connection not found", 404)
    result = await CalendarSync(db, settings, crypto).sync_connection(conn)
    return SyncResult(
        seen=int(result["seen"] or 0),
        upserted=int(result["upserted"] or 0),
        deleted=int(result["deleted"] or 0),
        last_synced_at=result["last_synced_at"],  # type: ignore[arg-type]
    )


@oauth_router.get("/auth/google/callback")
async def google_callback(
    db: DbSession,
    settings: AppSettings,
    crypto: Crypto,
    background: BackgroundTasks,
    code: str | None = Query(default=None),
    state: str | None = Query(default=None),
    error: str | None = Query(default=None),
) -> RedirectResponse:
    frontend = settings.google_frontend_redirect_url
    if error or not code or not state:
        # Never log code
        logger.warning("OAuth callback error or missing params")
        return RedirectResponse(f"{frontend}?oauth=error&reason=denied")
    try:
        oauth = GoogleOAuth(db, settings, crypto)
        redirect, conn_id = await oauth.handle_callback(code, state)
        # Kick off sync in background with a fresh session
        background.add_task(_bg_sync, conn_id, settings)
        return RedirectResponse(redirect)
    except AppError as exc:
        logger.info("OAuth callback failed: %s", exc.code)
        return RedirectResponse(f"{frontend}?oauth=error&reason={exc.code}")
    except Exception:
        logger.exception("OAuth callback unexpected failure")
        return RedirectResponse(f"{frontend}?oauth=error&reason=internal")


async def _bg_sync(conn_id: str, settings: object) -> None:
    from meetops_common.db import get_session_factory
    from meetops_common.settings import Settings, get_settings

    settings = get_settings() if not isinstance(settings, Settings) else settings
    factory = get_session_factory(settings)
    async with factory() as session:
        try:
            conn = await session.get(CalendarConnection, conn_id)
            if conn:
                crypto = TokenCrypto(settings)
                await CalendarSync(session, settings, crypto).sync_connection(conn)
            await session.commit()
        except Exception:
            await session.rollback()
            logger.exception("Background calendar sync failed conn=%s", conn_id)