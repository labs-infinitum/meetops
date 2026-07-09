"""Notes and todos routes."""

from __future__ import annotations

from datetime import UTC, datetime

from fastapi import APIRouter, Query
from sqlalchemy import select

from meetops_api.deps import CurrentUser, DbSession
from meetops_api.errors import AppError
from meetops_api.schemas import NoteOut, TodoOut, TodoPatch
from meetops_common.models import ActionItem, BotRun, Meeting, Summary, Transcript

router_notes = APIRouter(prefix="/notes", tags=["notes"])
router_todos = APIRouter(prefix="/todos", tags=["todos"])


@router_notes.get("/", response_model=list[NoteOut])
async def list_notes(user: CurrentUser, db: DbSession) -> list[NoteOut]:
    rows = await db.execute(
        select(Summary, Transcript, BotRun, Meeting)
        .join(Transcript, Transcript.id == Summary.transcript_id)
        .join(BotRun, BotRun.id == Transcript.bot_run_id)
        .outerjoin(Meeting, Meeting.id == BotRun.meeting_id)
        .where(BotRun.user_id == user.id)
        .order_by(Summary.created_at.desc())
        .limit(100)
    )
    out: list[NoteOut] = []
    for summary, transcript, run, meeting in rows:
        items = await db.scalars(select(ActionItem).where(ActionItem.summary_id == summary.id))
        out.append(
            NoteOut(
                id=str(summary.id),
                transcript_id=str(transcript.id),
                bot_run_id=str(run.id),
                status=summary.status,
                tldr=summary.tldr,
                summary_md=summary.summary_md,
                meeting_title=meeting.title if meeting else None,
                created_at=summary.created_at,
                action_items=[TodoOut.from_orm(i) for i in items],
            )
        )
    return out


@router_notes.get("/{transcript_id}", response_model=NoteOut)
async def get_note(transcript_id: str, user: CurrentUser, db: DbSession) -> NoteOut:
    try:
        tid = int(transcript_id)
    except ValueError as exc:
        raise AppError("not_found", "Note not found", 404) from exc
    row = await db.execute(
        select(Summary, Transcript, BotRun, Meeting)
        .join(Transcript, Transcript.id == Summary.transcript_id)
        .join(BotRun, BotRun.id == Transcript.bot_run_id)
        .outerjoin(Meeting, Meeting.id == BotRun.meeting_id)
        .where(Transcript.id == tid, BotRun.user_id == user.id)
    )
    first = row.first()
    if not first:
        # Also allow lookup by summary id for convenience
        row2 = await db.execute(
            select(Summary, Transcript, BotRun, Meeting)
            .join(Transcript, Transcript.id == Summary.transcript_id)
            .join(BotRun, BotRun.id == Transcript.bot_run_id)
            .outerjoin(Meeting, Meeting.id == BotRun.meeting_id)
            .where(Summary.id == tid, BotRun.user_id == user.id)
        )
        first = row2.first()
    if not first:
        raise AppError("not_found", "Note not found", 404)
    summary, transcript, run, meeting = first
    items = await db.scalars(select(ActionItem).where(ActionItem.summary_id == summary.id))
    return NoteOut(
        id=str(summary.id),
        transcript_id=str(transcript.id),
        bot_run_id=str(run.id),
        status=summary.status,
        tldr=summary.tldr,
        summary_md=summary.summary_md,
        meeting_title=meeting.title if meeting else None,
        created_at=summary.created_at,
        action_items=[TodoOut.from_orm(i) for i in items],
    )


@router_todos.get("/", response_model=list[TodoOut])
async def list_todos(
    user: CurrentUser,
    db: DbSession,
    completed: bool | None = Query(default=None),
) -> list[TodoOut]:
    q = select(ActionItem).where(ActionItem.user_id == user.id)
    if completed is not None:
        q = q.where(ActionItem.completed.is_(completed))
    q = q.order_by(ActionItem.created_at.desc()).limit(200)
    rows = await db.scalars(q)
    return [TodoOut.from_orm(i) for i in rows]


@router_todos.patch("/{action_item_id}", response_model=TodoOut)
async def patch_todo(
    action_item_id: str, body: TodoPatch, user: CurrentUser, db: DbSession
) -> TodoOut:
    try:
        aid = int(action_item_id)
    except ValueError as exc:
        raise AppError("not_found", "Todo not found", 404) from exc
    item = await db.get(ActionItem, aid)
    if item is None or item.user_id != user.id:
        raise AppError("not_found", "Todo not found", 404)
    item.completed = body.completed
    item.completed_at = datetime.now(UTC) if body.completed else None
    await db.flush()
    return TodoOut.from_orm(item)