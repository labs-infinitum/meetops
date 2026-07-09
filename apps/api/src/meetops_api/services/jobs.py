"""Job queue helpers (SKIP LOCKED)."""

from __future__ import annotations

from datetime import UTC, datetime, timedelta

from sqlalchemy import select, text, update
from sqlalchemy.ext.asyncio import AsyncSession

from meetops_common.models import Job

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


async def complete_job(db: AsyncSession, job: Job) -> None:
    job.status = "completed"
    job.locked_at = None
    job.locked_by = None
    await db.flush()


async def fail_job(db: AsyncSession, job: Job, error: str) -> None:
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