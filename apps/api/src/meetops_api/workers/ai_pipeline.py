"""AI pipeline worker: transcribe → clean → notes."""

from __future__ import annotations

import logging
import tempfile
from datetime import UTC, datetime
from pathlib import Path

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from meetops_api.ai.deepgram_client import DeepgramClient
from meetops_api.ai.languages import DEFAULT_LANGUAGES, normalize_languages
from meetops_api.ai.openai_client import CleanedTranscript, OpenAIClient
from meetops_api.ai.speaker_attribution import (
    apply_speaker_renames,
    extract_diarized_utterances,
    extract_roster_names,
    format_speaker_transcript,
    map_diarized_speakers,
)
from meetops_api.services.jobs import (
    claim_next_job,
    complete_job,
    enqueue_job,
    fail_job,
    is_job_cancelled,
)
from meetops_api.storage import get_storage
from meetops_common.models import (
    ActionItem,
    BotRun,
    Meeting,
    MeetingTimelineEvent,
    Summary,
    Transcript,
    UserSettings,
)
from meetops_common.settings import Settings

logger = logging.getLogger(__name__)

_warned_idle = False


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
        self.ai = OpenAIClient(settings)
        self.deepgram = DeepgramClient(settings)

    @property
    def transcription_enabled(self) -> bool:
        provider = self.settings.meetops_transcribe_provider
        if provider == "deepgram":
            return self.deepgram.enabled
        if provider == "openai":
            return self.ai.enabled
        return self.deepgram.enabled or self.ai.enabled

    @property
    def pipeline_enabled(self) -> bool:
        # Notes still need OpenAI; transcription can be Deepgram-only for diarized clean.
        return self.transcription_enabled or self.ai.enabled

    def _select_transcriber(self) -> str:
        provider = self.settings.meetops_transcribe_provider
        if provider == "deepgram":
            if not self.deepgram.enabled:
                raise RuntimeError("MEETOPS_TRANSCRIBE_PROVIDER=deepgram but DEEPGRAM_API_KEY unset")
            return "deepgram"
        if provider == "openai":
            if not self.ai.enabled:
                raise RuntimeError("MEETOPS_TRANSCRIBE_PROVIDER=openai but OPENAI_API_KEY unset")
            return "openai"
        if self.deepgram.enabled:
            return "deepgram"
        if self.ai.enabled:
            return "openai"
        raise RuntimeError("No transcription provider configured (set DEEPGRAM_API_KEY or OPENAI_API_KEY)")

    async def _languages_for_bot_run(self, bot_run_id: int | None) -> list[str]:
        if bot_run_id is None:
            return list(DEFAULT_LANGUAGES)
        async with self.session_factory() as session:
            run = await session.get(BotRun, bot_run_id)
            if run is None:
                return list(DEFAULT_LANGUAGES)
            settings_row = await session.get(UserSettings, run.user_id)
            await session.commit()
        if settings_row is None:
            return list(DEFAULT_LANGUAGES)
        try:
            return normalize_languages(settings_row.languages_json)
        except ValueError:
            return list(DEFAULT_LANGUAGES)

    async def tick(self) -> None:
        global _warned_idle
        if not self.pipeline_enabled:
            if not _warned_idle:
                logger.warning(
                    "No DEEPGRAM_API_KEY / OPENAI_API_KEY; AI pipeline idle"
                )
                _warned_idle = True
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
                from meetops_common.models import Job

                job = await session.get(Job, job_id)
                if job and job.status != "cancelled":
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

        languages = await self._languages_for_bot_run(bot_run_id)

        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / "audio.webm"
            storage = get_storage(self.settings)
            if Path(path_key).exists():
                dest.write_bytes(Path(path_key).read_bytes())
            elif storage is not None:
                await __import__("asyncio").to_thread(storage.download_file, path_key, dest)
            else:
                raise RuntimeError("Cannot locate recording for transcription")

            provider = self._select_transcriber()
            if provider == "deepgram":
                result = await self.deepgram.transcribe(dest, languages=languages)
            else:
                result = await self.ai.transcribe(dest, languages=languages)

        async with self.session_factory() as session:
            if await is_job_cancelled(session, job_id):
                await session.commit()
                logger.info("transcribe abandoned (cancelled) bot_run=%s", bot_run_id)
                return
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
            logger.info(
                "transcribed bot_run=%s provider=%s",
                bot_run_id,
                provider,
            )

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
                    MeetingTimelineEvent.event_type.in_(
                        (
                            "active_speaker_changed",
                            "active_speaker_sample",
                            "participant_snapshot",
                            "participant_joined",
                        )
                    ),
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

        languages = await self._languages_for_bot_run(brid)
        diarized = extract_diarized_utterances(segments)
        if diarized is not None:
            attribution = map_diarized_speakers(diarized, timeline)
            speaker_turns = attribution.turns
            roster = attribution.roster or extract_roster_names(timeline)
            if attribution.needs_llm_fallback and self.ai.enabled and roster:
                try:
                    renames = await self.ai.rename_speakers(
                        turns=speaker_turns,
                        roster=roster,
                        timeline=timeline,
                        languages=languages,
                    )
                    if renames:
                        speaker_turns = apply_speaker_renames(
                            speaker_turns, renames, roster=roster
                        )
                        attribution.method = "overlap+llm"
                except Exception:
                    logger.exception(
                        "speaker rename fallback failed bot_run=%s", brid
                    )
            logger.info(
                "speaker attribution bot_run=%s mapped=%s unmapped=%s method=%s roster=%s",
                brid,
                attribution.mapped,
                attribution.unmapped,
                attribution.method,
                len(roster),
            )
            cleaned = CleanedTranscript(
                cleaned_text=format_speaker_transcript(speaker_turns),
                speaker_turns=speaker_turns,
            )
        else:
            if not self.ai.enabled:
                raise RuntimeError(
                    "Non-diarized transcript requires OPENAI_API_KEY for clean_transcript"
                )
            cleaned = await self.ai.clean_transcript(
                raw_text=raw,
                segments=segments if isinstance(segments, list) else None,
                timeline=timeline,
                languages=languages,
            )

        async with self.session_factory() as session:
            if await is_job_cancelled(session, job_id):
                await session.commit()
                logger.info("clean abandoned (cancelled) transcript=%s", tid)
                return
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

        if not self.ai.enabled:
            raise RuntimeError("OPENAI_API_KEY required for generate_meeting_notes")

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

        languages = await self._languages_for_bot_run(brid)
        notes = await self.ai.generate_notes(
            cleaned_text=text, title=title, languages=languages
        )

        async with self.session_factory() as session:
            if await is_job_cancelled(session, job_id):
                await session.commit()
                logger.info("notes abandoned (cancelled) transcript=%s", tid)
                return
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
