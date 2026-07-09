"""Recordings / bot runs routes."""

from __future__ import annotations

import asyncio
from datetime import UTC, datetime

from fastapi import APIRouter, Query
from sqlalchemy import select

from meetops_api.deps import AppSettings, CurrentUser, DbSession
from meetops_api.errors import AppError
from meetops_api.schemas import (
    AdHocRequest,
    BotRunOut,
    PlaybackUrlOut,
    TimelineEventOut,
)
from meetops_api.services.meet_url import normalize_meet_url
from meetops_api.storage.s3 import S3Storage
from meetops_common.constants import TERMINAL_BOT_STATUSES
from meetops_common.models import BotRun, MeetingTimelineEvent

router = APIRouter(prefix="/recordings", tags=["recordings"])

# In-process stop events keyed by run id (worker also polls DB status)
_stop_events: dict[int, asyncio.Event] = {}


def get_stop_event(run_id: int) -> asyncio.Event:
    if run_id not in _stop_events:
        _stop_events[run_id] = asyncio.Event()
    return _stop_events[run_id]


def clear_stop_event(run_id: int) -> None:
    _stop_events.pop(run_id, None)


def signal_stop(run_id: int) -> None:
    get_stop_event(run_id).set()


@router.get("/", response_model=list[BotRunOut])
async def list_recordings(
    user: CurrentUser,
    db: DbSession,
    limit: int = Query(default=50, ge=1, le=200),
) -> list[BotRunOut]:
    rows = await db.scalars(
        select(BotRun)
        .where(BotRun.user_id == user.id)
        .order_by(BotRun.created_at.desc())
        .limit(limit)
    )
    return [BotRunOut.from_orm_run(r) for r in rows]


@router.get("/{run_id}", response_model=BotRunOut)
async def get_recording(run_id: str, user: CurrentUser, db: DbSession) -> BotRunOut:
    run = await _get_owned_run(db, user.id, run_id)
    return BotRunOut.from_orm_run(run)


@router.get("/{run_id}/playback-url", response_model=PlaybackUrlOut)
async def playback_url(
    run_id: str, user: CurrentUser, db: DbSession, settings: AppSettings
) -> PlaybackUrlOut:
    run = await _get_owned_run(db, user.id, run_id)
    if not run.recording_path:
        raise AppError("not_ready", "Recording not available", 404)
    if not settings.s3_bucket:
        raise AppError("storage_not_configured", "S3 is not configured", 503)
    url, expires = S3Storage(settings).presign_get(run.recording_path)
    return PlaybackUrlOut(url=url, expires_at=expires)


@router.get("/{run_id}/timeline", response_model=list[TimelineEventOut])
async def timeline(
    run_id: str,
    user: CurrentUser,
    db: DbSession,
    limit: int = Query(default=500, ge=1, le=5000),
) -> list[TimelineEventOut]:
    run = await _get_owned_run(db, user.id, run_id)
    rows = await db.scalars(
        select(MeetingTimelineEvent)
        .where(MeetingTimelineEvent.bot_run_id == run.id)
        .order_by(MeetingTimelineEvent.occurred_at.asc())
        .limit(limit)
    )
    return [TimelineEventOut.from_orm(e) for e in rows]


@router.post("/adhoc", response_model=BotRunOut)
async def start_adhoc(
    body: AdHocRequest, user: CurrentUser, db: DbSession
) -> BotRunOut:
    meet_url = normalize_meet_url(body.meet_url)
    run = BotRun(
        user_id=user.id,
        status="queued",
        trigger="adhoc",
        meet_url=meet_url,
        meeting_start_at=datetime.now(UTC),
    )
    db.add(run)
    await db.flush()
    # Register stop channel at claim/create time
    get_stop_event(run.id)
    return BotRunOut.from_orm_run(run)


@router.post("/{run_id}/stop", response_model=BotRunOut)
async def stop_recording(run_id: str, user: CurrentUser, db: DbSession) -> BotRunOut:
    run = await _get_owned_run(db, user.id, run_id)
    if run.status in TERMINAL_BOT_STATUSES:
        return BotRunOut.from_orm_run(run)
    if run.status == "queued":
        run.status = "cancelled"
        run.error = "cancelled_by_user"
        run.ended_at = datetime.now(UTC)
    else:
        # Persist stop intent in DB so the worker process (separate container) can see it.
        # In-process Event only helps when API and worker share memory (they do not in Compose).
        run.error = "stop_requested"
        signal_stop(run.id)
    await db.flush()
    return BotRunOut.from_orm_run(run)


async def _get_owned_run(db: object, user_id: str, run_id: str) -> BotRun:
    try:
        rid = int(run_id)
    except ValueError as exc:
        raise AppError("not_found", "Recording not found", 404) from exc
    run = await db.get(BotRun, rid)  # type: ignore[attr-defined]
    if run is None or run.user_id != user_id:
        raise AppError("not_found", "Recording not found", 404)
    return run