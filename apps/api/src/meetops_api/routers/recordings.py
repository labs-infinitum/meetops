"""Recordings / bot runs routes."""

from __future__ import annotations

import asyncio
from datetime import UTC, datetime, timedelta

from fastapi import APIRouter, Query
from fastapi.responses import FileResponse
from sqlalchemy import select

from meetops_api.deps import AppSettings, CurrentUser, DbSession
from meetops_api.errors import AppError
from meetops_api.schemas import (
    AdHocRequest,
    BotRunOut,
    NoteOut,
    PlaybackUrlOut,
    TimelineEventOut,
    TodoOut,
)
from meetops_api.services.meet_url import normalize_meet_url
from meetops_api.storage import LocalStorage, get_storage
from meetops_common.constants import TERMINAL_BOT_STATUSES
from meetops_common.models import (
    ActionItem,
    BotRun,
    Meeting,
    MeetingTimelineEvent,
    Summary,
    Transcript,
)

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


async def _meeting_title_map(db: DbSession, meeting_ids: list[str]) -> dict[str, str]:
    ids = [mid for mid in meeting_ids if mid]
    if not ids:
        return {}
    rows = await db.scalars(select(Meeting).where(Meeting.id.in_(ids)))
    return {m.id: m.title for m in rows}


def _to_out(run: BotRun, titles: dict[str, str]) -> BotRunOut:
    title = titles.get(run.meeting_id) if run.meeting_id else None
    return BotRunOut.from_orm_run(run, meeting_title=title)


@router.get("/", response_model=list[BotRunOut])
async def list_recordings(
    user: CurrentUser,
    db: DbSession,
    limit: int = Query(default=50, ge=1, le=200),
) -> list[BotRunOut]:
    rows = list(
        await db.scalars(
            select(BotRun)
            .where(BotRun.user_id == user.id)
            .order_by(BotRun.created_at.desc())
            .limit(limit)
        )
    )
    titles = await _meeting_title_map(db, [r.meeting_id for r in rows if r.meeting_id])
    return [_to_out(r, titles) for r in rows]


@router.get("/{run_id}", response_model=BotRunOut)
async def get_recording(run_id: str, user: CurrentUser, db: DbSession) -> BotRunOut:
    run = await _get_owned_run(db, user.id, run_id)
    titles = await _meeting_title_map(db, [run.meeting_id] if run.meeting_id else [])
    return _to_out(run, titles)


@router.get("/{run_id}/playback-url", response_model=PlaybackUrlOut)
async def playback_url(
    run_id: str, user: CurrentUser, db: DbSession, settings: AppSettings
) -> PlaybackUrlOut:
    run = await _get_owned_run(db, user.id, run_id)
    if not run.recording_path:
        raise AppError("not_ready", "Recording not available", 404)
    storage = get_storage(settings)
    if storage is None:
        raise AppError("storage_not_configured", "Recording storage is not configured", 503)
    if isinstance(storage, LocalStorage):
        # Streamed by /media below (same origin, session cookie); the expiry is nominal.
        url = f"{settings.meetops_public_api_url.rstrip('/')}/api/v1/recordings/{run_id}/media"
        expires = datetime.now(UTC) + timedelta(seconds=settings.meetops_presigned_url_ttl_secs)
        return PlaybackUrlOut(url=url, expires_at=expires)
    url, expires = storage.presign_get(run.recording_path)
    return PlaybackUrlOut(url=url, expires_at=expires)


@router.get("/{run_id}/media", response_class=FileResponse)
async def recording_media(
    run_id: str, user: CurrentUser, db: DbSession, settings: AppSettings
) -> FileResponse:
    """The recording file itself, for MEETOPS_STORAGE_BACKEND=local (supports Range for seeking)."""
    run = await _get_owned_run(db, user.id, run_id)
    if settings.meetops_storage_backend != "local" or not run.recording_path:
        raise AppError("not_found", "Recording not available", 404)
    try:
        path = LocalStorage(settings).path_for(run.recording_path)
    except ValueError:
        raise AppError("not_found", "Recording not available", 404) from None
    if not path.is_file():
        raise AppError("not_found", "Recording not available", 404)
    return FileResponse(path, media_type="video/webm")


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


@router.get("/{run_id}/ai", response_model=NoteOut | None)
async def recording_ai(run_id: str, user: CurrentUser, db: DbSession) -> NoteOut | None:
    """Transcript + notes for a recording (null if AI pipeline has not started)."""
    run = await _get_owned_run(db, user.id, run_id)
    return await _note_for_run(db, run)


@router.post("/{run_id}/ai/rerun", response_model=NoteOut)
async def rerun_recording_ai(run_id: str, user: CurrentUser, db: DbSession) -> NoteOut:
    """Re-run transcribe → clean → notes for a completed recording."""
    from meetops_api.services.jobs import requeue_ai_pipeline

    run = await _get_owned_run(db, user.id, run_id)
    if run.status != "completed":
        raise AppError(
            "not_ready",
            "AI can only be re-run after the recording has completed",
            409,
        )
    if not (run.audio_path or run.recording_path):
        raise AppError("not_ready", "Recording media is not available", 409)

    await requeue_ai_pipeline(db, run.id)
    await db.flush()
    note = await _note_for_run(db, run)
    if note is None:
        raise AppError("internal_error", "Failed to start AI pipeline", 500)
    return note


async def _note_for_run(db: DbSession, run: BotRun) -> NoteOut | None:
    transcript = await db.scalar(select(Transcript).where(Transcript.bot_run_id == run.id))
    if transcript is None:
        return None
    summary = await db.scalar(
        select(Summary).where(Summary.transcript_id == transcript.id)
    )
    meeting = None
    if run.meeting_id:
        meeting = await db.get(Meeting, run.meeting_id)
    items: list[ActionItem] = []
    if summary is not None:
        items = list(
            await db.scalars(select(ActionItem).where(ActionItem.summary_id == summary.id))
        )
    return NoteOut(
        id=str(summary.id) if summary else str(transcript.id),
        transcript_id=str(transcript.id),
        bot_run_id=str(run.id),
        status=summary.status if summary else transcript.status,
        transcript_status=transcript.status,
        summary_status=summary.status if summary else None,
        tldr=summary.tldr if summary else None,
        summary_md=summary.summary_md if summary else None,
        raw_transcript=transcript.raw_text,
        cleaned_transcript=transcript.cleaned_text,
        meeting_title=meeting.title if meeting else None,
        error=(summary.error if summary else None) or transcript.error,
        created_at=summary.created_at if summary else transcript.created_at,
        action_items=[TodoOut.from_orm(i) for i in items],
    )


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
    return BotRunOut.from_orm_run(run, meeting_title=None)


@router.post("/{run_id}/stop", response_model=BotRunOut)
async def stop_recording(run_id: str, user: CurrentUser, db: DbSession) -> BotRunOut:
    run = await _get_owned_run(db, user.id, run_id)
    if run.status in TERMINAL_BOT_STATUSES:
        titles = await _meeting_title_map(db, [run.meeting_id] if run.meeting_id else [])
        return _to_out(run, titles)
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
    titles = await _meeting_title_map(db, [run.meeting_id] if run.meeting_id else [])
    return _to_out(run, titles)


async def _get_owned_run(db: object, user_id: str, run_id: str) -> BotRun:
    try:
        rid = int(run_id)
    except ValueError as exc:
        raise AppError("not_found", "Recording not found", 404) from exc
    run = await db.get(BotRun, rid)  # type: ignore[attr-defined]
    if run is None or run.user_id != user_id:
        raise AppError("not_found", "Recording not found", 404)
    return run
