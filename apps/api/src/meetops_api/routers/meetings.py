"""Meetings + dashboard routes."""

from __future__ import annotations

from datetime import UTC, datetime

from fastapi import APIRouter, Query
from sqlalchemy import func, select

from meetops_api.deps import CurrentUser, DbSession
from meetops_api.errors import AppError
from meetops_api.schemas import DashboardOut, MeetingOut, MeetingPatch, TodoOut
from meetops_common.models import ActionItem, BotRun, Meeting, Summary, Transcript

router = APIRouter(prefix="/meetings", tags=["meetings"])


def _utcnow() -> datetime:
    return datetime.now(UTC)


@router.get("/", response_model=list[MeetingOut])
async def list_meetings(
    user: CurrentUser,
    db: DbSession,
    limit: int = Query(default=50, ge=1, le=200),
) -> list[MeetingOut]:
    rows = await db.scalars(
        select(Meeting)
        .where(Meeting.user_id == user.id)
        .order_by(Meeting.start_at.asc())
        .limit(limit)
    )
    return [MeetingOut.model_validate(m) for m in rows]


@router.get("/dashboard", response_model=DashboardOut)
async def dashboard(user: CurrentUser, db: DbSession) -> DashboardOut:
    now = _utcnow()
    upcoming = list(
        await db.scalars(
            select(Meeting)
            .where(Meeting.user_id == user.id, Meeting.end_at >= now)
            .order_by(Meeting.start_at.asc())
            .limit(10)
        )
    )
    upcoming_count = await db.scalar(
        select(func.count()).select_from(Meeting).where(
            Meeting.user_id == user.id, Meeting.end_at >= now
        )
    ) or 0
    recordings_count = await db.scalar(
        select(func.count()).select_from(BotRun).where(BotRun.user_id == user.id)
    ) or 0
    notes_count = await db.scalar(
        select(func.count())
        .select_from(Summary)
        .join(Transcript, Transcript.id == Summary.transcript_id)
        .join(BotRun, BotRun.id == Transcript.bot_run_id)
        .where(BotRun.user_id == user.id, Summary.status == "completed")
    ) or 0
    open_todos = await db.scalar(
        select(func.count()).select_from(ActionItem).where(
            ActionItem.user_id == user.id, ActionItem.completed.is_(False)
        )
    ) or 0

    latest_summary = None
    row = await db.execute(
        select(Summary, Transcript, BotRun, Meeting)
        .join(Transcript, Transcript.id == Summary.transcript_id)
        .join(BotRun, BotRun.id == Transcript.bot_run_id)
        .outerjoin(Meeting, Meeting.id == BotRun.meeting_id)
        .where(BotRun.user_id == user.id, Summary.status == "completed")
        .order_by(Summary.created_at.desc())
        .limit(1)
    )
    first = row.first()
    if first:
        summary, transcript, run, meeting = first
        items = await db.scalars(
            select(ActionItem).where(ActionItem.summary_id == summary.id)
        )
        latest_summary = {
            "id": str(summary.id),
            "tldr": summary.tldr,
            "summary_md": summary.summary_md,
            "meeting_title": meeting.title if meeting else None,
            "bot_run_id": str(run.id),
            "created_at": summary.created_at.isoformat(),
            "action_items": [TodoOut.from_orm(i).model_dump(mode="json") for i in items],
        }

    return DashboardOut(
        upcoming_count=int(upcoming_count),
        recordings_count=int(recordings_count),
        notes_count=int(notes_count),
        open_todos_count=int(open_todos),
        upcoming=[MeetingOut.model_validate(m) for m in upcoming],
        latest_summary=latest_summary,
    )


@router.get("/{meeting_id}", response_model=MeetingOut)
async def get_meeting(meeting_id: str, user: CurrentUser, db: DbSession) -> MeetingOut:
    meeting = await db.get(Meeting, meeting_id)
    if meeting is None or meeting.user_id != user.id:
        raise AppError("not_found", "Meeting not found", 404)
    return MeetingOut.model_validate(meeting)


@router.patch("/{meeting_id}", response_model=MeetingOut)
async def patch_meeting(
    meeting_id: str, body: MeetingPatch, user: CurrentUser, db: DbSession
) -> MeetingOut:
    meeting = await db.get(Meeting, meeting_id)
    if meeting is None or meeting.user_id != user.id:
        raise AppError("not_found", "Meeting not found", 404)
    meeting.auto_join = body.auto_record
    await db.flush()
    return MeetingOut.model_validate(meeting)