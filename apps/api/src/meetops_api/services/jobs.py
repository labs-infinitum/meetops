"""Job queue helpers (SKIP LOCKED)."""

from __future__ import annotations

from datetime import UTC, datetime, timedelta

from sqlalchemy import select, text, update
from sqlalchemy.ext.asyncio import AsyncSession

from meetops_common.constants import JOB_KINDS
from meetops_common.models import ActionItem, Job, Summary, Transcript

BACKOFF_SECS = (30, 120, 300, 900)
MAX_ATTEMPTS = 3


def _utcnow() -> datetime:
    return datetime.now(UTC)


async def enqueue_job(
    db: AsyncSession,
    *,
    kind: str,
    bot_run_id: int | None = None,
    meeting_id: str | None = None,
    transcript_id: int | None = None,
    payload: dict | None = None,
) -> Job | None:
    # Dedup active/completed for same kind+bot_run
    if bot_run_id is not None:
        existing = await db.scalar(
            select(Job.id).where(
                Job.kind == kind,
                Job.bot_run_id == bot_run_id,
                Job.status.in_(("queued", "running", "completed")),
            )
        )
        if existing:
            return None
    job = Job(
        kind=kind,
        status="queued",
        bot_run_id=bot_run_id,
        meeting_id=meeting_id,
        transcript_id=transcript_id,
        payload_json=payload,
        run_after=_utcnow(),
    )
    db.add(job)
    await db.flush()
    return job


async def cancel_ai_jobs_for_bot_run(db: AsyncSession, bot_run_id: int) -> int:
    """Supersede queued/running/completed AI jobs so a fresh pipeline can enqueue."""
    result = await db.execute(
        update(Job)
        .where(
            Job.bot_run_id == bot_run_id,
            Job.kind.in_(JOB_KINDS),
            Job.status.in_(("queued", "running", "completed")),
        )
        .values(
            status="cancelled",
            locked_at=None,
            locked_by=None,
            last_error="superseded_by_rerun",
        )
    )
    return int(result.rowcount or 0)


async def requeue_ai_pipeline(db: AsyncSession, bot_run_id: int) -> tuple[Job, Transcript]:
    """Reset AI artifacts and enqueue a fresh transcribe → clean → notes chain."""
    await cancel_ai_jobs_for_bot_run(db, bot_run_id)

    transcript = await db.scalar(
        select(Transcript).where(Transcript.bot_run_id == bot_run_id)
    )
    if transcript is None:
        transcript = Transcript(bot_run_id=bot_run_id, status="processing")
        db.add(transcript)
        await db.flush()
    else:
        transcript.status = "processing"
        transcript.error = None
        transcript.raw_text = None
        transcript.cleaned_text = None
        transcript.segments_json = None
        transcript.cleaned_segments_json = None
        transcript.language = None

    summary = await db.scalar(
        select(Summary).where(Summary.transcript_id == transcript.id)
    )
    if summary is not None:
        summary.status = "processing"
        summary.tldr = None
        summary.summary_md = None
        summary.error = None
        items = await db.scalars(
            select(ActionItem).where(ActionItem.summary_id == summary.id)
        )
        for item in items:
            await db.delete(item)

    job = await enqueue_job(
        db,
        kind="transcribe_recording",
        bot_run_id=bot_run_id,
        transcript_id=transcript.id,
    )
    if job is None:
        raise RuntimeError("Failed to enqueue transcription after cancelling prior jobs")
    await db.flush()
    return job, transcript


async def claim_next_job(db: AsyncSession, worker_id: str) -> Job | None:
    now = _utcnow()
    result = await db.execute(
        text(
            """
            SELECT id FROM jobs
            WHERE status = 'queued' AND run_after <= :now
            ORDER BY run_after ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
            """
        ),
        {"now": now},
    )
    row = result.first()
    if not row:
        return None
    job_id = row[0]
    await db.execute(
        update(Job)
        .where(Job.id == job_id)
        .values(
            status="running",
            locked_at=now,
            locked_by=worker_id,
            attempts=Job.attempts + 1,
        )
    )
    await db.flush()
    return await db.get(Job, job_id)


async def is_job_cancelled(db: AsyncSession, job_id: int) -> bool:
    job = await db.get(Job, job_id)
    return job is None or job.status == "cancelled"


async def complete_job(db: AsyncSession, job: Job) -> bool:
    """Mark job completed. Returns False if the job was cancelled (no-op)."""
    if job.status == "cancelled":
        return False
    job.status = "completed"
    job.locked_at = None
    job.locked_by = None
    await db.flush()
    return True


async def fail_job(db: AsyncSession, job: Job, error: str) -> None:
    if job.status == "cancelled":
        return
    job.last_error = error[:2000]
    if job.attempts >= MAX_ATTEMPTS:
        job.status = "failed"
        job.locked_at = None
        job.locked_by = None
    else:
        delay = BACKOFF_SECS[min(job.attempts - 1, len(BACKOFF_SECS) - 1)]
        job.status = "queued"
        job.run_after = _utcnow() + timedelta(seconds=delay)
        job.locked_at = None
        job.locked_by = None
    await db.flush()