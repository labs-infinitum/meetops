"""AI pipeline worker: transcribe → clean → notes."""

from __future__ import annotations

import logging
import tempfile
from datetime import UTC, datetime
from pathlib import Path

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from meetops_api.ai.groq import GroqClient
from meetops_api.services.jobs import claim_next_job, complete_job, enqueue_job, fail_job
from meetops_api.storage.s3 import S3Storage
from meetops_common.models import (
    ActionItem,
    BotRun,
    Meeting,
    MeetingTimelineEvent,
    Summary,
    Transcript,
)
from meetops_common.settings import Settings

logger = logging.getLogger(__name__)

_warned_no_key = False


def _utcnow() -> datetime:
    return datetime.now(UTC)


class AIPipeline:
    def __init__(
        self,
        settings: Settings,
        session_factory: async_sessionmaker[AsyncSession],
        worker_id: str = "ai-worker",
    ) -> None:
        self.settings = settings
        self.session_factory = session_factory
        self.worker_id = worker_id
        self.groq = GroqClient(settings)

    async def tick(self) -> None:
        global _warned_no_key
        if not self.groq.enabled:
            if not _warned_no_key:
                logger.warning("GROQ_API_KEY missing; AI pipeline idle")
                _warned_no_key = True
            return

        async with self.session_factory() as session:
            job = await claim_next_job(session, self.worker_id)
            if job is None:
                await session.commit()
                return
            await session.commit()
            job_id = job.id
            kind = job.kind
            bot_run_id = job.bot_run_id
            transcript_id = job.transcript_id

        try:
            if kind == "transcribe_recording":
                await self._transcribe(job_id, bot_run_id)
            elif kind == "clean_transcript":
                await self._clean(job_id, transcript_id, bot_run_id)
            elif kind == "generate_meeting_notes":
                await self._notes(job_id, transcript_id, bot_run_id)
            else:
                raise RuntimeError(f"Unknown job kind: {kind}")
        except Exception as exc:
            logger.exception("AI job failed id=%s kind=%s", job_id, kind)
            async with self.session_factory() as session:
                job = await session.get(type(job), job_id) if False else None
                from meetops_common.models import Job

                job = await session.get(Job, job_id)
                if job:
                    await fail_job(session, job, str(exc))
                await session.commit()

    async def _transcribe(self, job_id: int, bot_run_id: int | None) -> None:
        from meetops_common.models import Job

        if bot_run_id is None:
            raise RuntimeError("transcribe job missing bot_run_id")
        async with self.session_factory() as session:
            run = await session.get(BotRun, bot_run_id)
            if run is None:
                raise RuntimeError("bot run not found")
            path_key = run.audio_path or run.recording_path
            await session.commit()

        if not path_key:
            raise RuntimeError("No recording path for transcription")

        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / "audio.webm"
            if Path(path_key).exists():
                dest.write_bytes(Path(path_key).read_bytes())
            elif self.settings.s3_bucket:
                await __import__("asyncio").to_thread(
                    S3Storage(self.settings).download_file, path_key, dest
                )
            else:
                raise RuntimeError("Cannot locate recording for transcription")

            result = await self.groq.transcribe(dest)

        async with self.session_factory() as session:
            transcript = await session.scalar(
                select(Transcript).where(Transcript.bot_run_id == bot_run_id)
            )
            if transcript is None:
                transcript = Transcript(bot_run_id=bot_run_id, status="processing")
                session.add(transcript)
                await session.flush()
            transcript.raw_text = result.get("text")
            transcript.segments_json = result.get("segments")
            transcript.language = result.get("language")
            transcript.status = "transcribed"
            await enqueue_job(
                session,
                kind="clean_transcript",
                bot_run_id=bot_run_id,
                transcript_id=transcript.id,
            )
            job = await session.get(Job, job_id)
            if job:
                await complete_job(session, job)
            await session.commit()

    async def _clean(
        self, job_id: int, transcript_id: int | None, bot_run_id: int | None
    ) -> None:
        from meetops_common.models import Job

        async with self.session_factory() as session:
            if transcript_id:
                transcript = await session.get(Transcript, transcript_id)
            elif bot_run_id:
                transcript = await session.scalar(
                    select(Transcript).where(Transcript.bot_run_id == bot_run_id)
                )
            else:
                transcript = None
            if transcript is None:
                raise RuntimeError("transcript not found")
            raw = transcript.raw_text or ""
            segments = transcript.segments_json
            tid = transcript.id
            brid = transcript.bot_run_id
            events = await session.scalars(
                select(MeetingTimelineEvent).where(
                    MeetingTimelineEvent.bot_run_id == brid,
                    MeetingTimelineEvent.event_type == "active_speaker_changed",
                )
            )
            timeline = [
                {
                    "type": e.event_type,
                    "relative_ms": e.relative_ms,
                    "participant_name": e.participant_name,
                    "payload": e.payload_json,
                }
                for e in events
            ]
            await session.commit()

        cleaned = await self.groq.clean_transcript(
            raw_text=raw, segments=segments if isinstance(segments, list) else None, timeline=timeline
        )

        async with self.session_factory() as session:
            transcript = await session.get(Transcript, tid)
            if transcript:
                transcript.cleaned_text = cleaned.cleaned_text
                transcript.cleaned_segments_json = cleaned.speaker_turns
                transcript.status = "cleaned"
                await enqueue_job(
                    session,
                    kind="generate_meeting_notes",
                    bot_run_id=brid,
                    transcript_id=tid,
                )
            job = await session.get(Job, job_id)
            if job:
                await complete_job(session, job)
            await session.commit()

    async def _notes(
        self, job_id: int, transcript_id: int | None, bot_run_id: int | None
    ) -> None:
        from meetops_common.models import Job

        async with self.session_factory() as session:
            if transcript_id:
                transcript = await session.get(Transcript, transcript_id)
            else:
                transcript = await session.scalar(
                    select(Transcript).where(Transcript.bot_run_id == bot_run_id)
                )
            if transcript is None:
                raise RuntimeError("transcript not found")
            text = transcript.cleaned_text or transcript.raw_text or ""
            tid = transcript.id
            brid = transcript.bot_run_id
            run = await session.get(BotRun, brid)
            user_id = run.user_id if run else None
            title = None
            if run and run.meeting_id:
                meeting = await session.get(Meeting, run.meeting_id)
                title = meeting.title if meeting else None
            await session.commit()

        notes = await self.groq.generate_notes(cleaned_text=text, title=title)

        async with self.session_factory() as session:
            summary = await session.scalar(
                select(Summary).where(Summary.transcript_id == tid)
            )
            if summary is None:
                summary = Summary(transcript_id=tid, status="processing")
                session.add(summary)
                await session.flush()
            summary.tldr = notes.tldr
            summary.summary_md = notes.summary_md
            summary.status = "completed"
            # Replace action items
            existing = await session.scalars(
                select(ActionItem).where(ActionItem.summary_id == summary.id)
            )
            for item in existing:
                await session.delete(item)
            if user_id:
                for draft in notes.action_items:
                    session.add(
                        ActionItem(
                            summary_id=summary.id,
                            bot_run_id=brid,
                            user_id=user_id,
                            text=draft.text,
                            owner=draft.owner,
                            due=draft.due,
                        )
                    )
            job = await session.get(Job, job_id)
            if job:
                await complete_job(session, job)
            await session.commit()