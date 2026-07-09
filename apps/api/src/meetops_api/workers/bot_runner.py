"""Bot run orchestration: claim slot, record, upload, enqueue AI."""

from __future__ import annotations

import asyncio
import logging
import shutil
from datetime import UTC, datetime
from pathlib import Path

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from meetops_api.bot.recorder import MeetRecorder
from meetops_api.bot.slots import SlotAllocator
from meetops_api.routers.recordings import clear_stop_event, get_stop_event
from meetops_api.services.jobs import enqueue_job
from meetops_api.storage.s3 import S3Storage
from meetops_common.constants import TERMINAL_BOT_STATUSES
from meetops_common.models import BotRun, Meeting, UserSettings
from meetops_common.settings import Settings

logger = logging.getLogger(__name__)


def _utcnow() -> datetime:
    return datetime.now(UTC)


class BotRunner:
    def __init__(
        self,
        settings: Settings,
        session_factory: async_sessionmaker[AsyncSession],
        allocator: SlotAllocator,
    ) -> None:
        self.settings = settings
        self.session_factory = session_factory
        self.allocator = allocator
        self._tasks: dict[int, asyncio.Task[None]] = {}
        self._accepting = True

    def stop_accepting(self) -> None:
        self._accepting = False

    @property
    def active_count(self) -> int:
        return len(self._tasks)

    async def reconcile_abandoned(self) -> None:
        async with self.session_factory() as session:
            rows = await session.scalars(
                select(BotRun).where(
                    BotRun.status.in_(
                        ("joining", "in_meeting", "recording", "uploading")
                    )
                )
            )
            for run in rows:
                run.status = "failed"
                run.error = "process_restarted"
                run.ended_at = _utcnow()
            await session.commit()

    async def spawn_if_needed(self, run_id: int) -> None:
        if not self._accepting:
            return
        if run_id in self._tasks:
            return
        # Register stop channel at claim time
        get_stop_event(run_id)
        task = asyncio.create_task(self._execute(run_id), name=f"bot-run-{run_id}")
        self._tasks[run_id] = task

        def _done(t: asyncio.Task[None]) -> None:
            self._tasks.pop(run_id, None)
            clear_stop_event(run_id)
            if t.cancelled():
                return
            exc = t.exception()
            if exc:
                logger.error("Bot task failed run=%s: %s", run_id, exc)

        task.add_done_callback(_done)

    async def signal_all_stop(self) -> None:
        for run_id in list(self._tasks):
            get_stop_event(run_id).set()

    async def wait_active(self, timeout: float = 120.0) -> None:
        if not self._tasks:
            return
        await asyncio.wait(
            list(self._tasks.values()),
            timeout=timeout,
            return_when=asyncio.ALL_COMPLETED,
        )

    async def _execute(self, run_id: int) -> None:
        stop_event = get_stop_event(run_id)
        if stop_event.is_set():
            await self._finalize(run_id, "cancelled", "cancelled_by_user")
            return

        async with self.session_factory() as session:
            run = await session.get(BotRun, run_id)
            if run is None or run.status in TERMINAL_BOT_STATUSES:
                return
            if run.status == "cancelled":
                return
            meet_url = run.meet_url
            meeting = None
            if run.meeting_id:
                meeting = await session.get(Meeting, run.meeting_id)
                if meeting and not meet_url:
                    meet_url = meeting.meet_url
            settings_row = await session.get(UserSettings, run.user_id)
            display_name = (
                (settings_row.bot_display_name if settings_row else None)
                or self.settings.meetops_bot_display_name
            )
            end_at = meeting.end_at if meeting else None
            max_dur = (
                self.settings.meetops_bot_adhoc_duration_secs
                if run.trigger == "adhoc"
                else None
            )
            user_id = run.user_id
            await session.commit()

        if not meet_url:
            await self._finalize(run_id, "failed", "missing_meet_url")
            return

        slot = await self.allocator.acquire(run_id)
        staging = (
            self.settings.staging_dir / user_id / str(run_id)
        )
        try:
            async def on_status(status: str) -> None:
                await self._update_status(run_id, status, slot=slot)

            recorder = MeetRecorder(
                self.settings,
                slot,
                session_factory=self.session_factory,
                bot_run_id=run_id,
                meet_url=meet_url,
                display_name=display_name,
                stop_event=stop_event,
                staging_dir=staging,
                on_status=on_status,
                end_at=end_at,
                max_duration_secs=max_dur,
            )
            await self._update_status(run_id, "joining", slot=slot)
            raw_path = await recorder.run()
            await self._update_status(run_id, "uploading")
            # Clear stop_requested so completed runs don't show it as an error banner
            await self._clear_stop_error(run_id)
            await self._upload_and_complete(run_id, user_id, raw_path, staging)
        except Exception as exc:
            logger.exception("Bot run failed id=%s", run_id)
            err = str(exc)[:500]
            if "cancelled" in err.lower() or "stop_requested" in err.lower():
                await self._finalize(run_id, "cancelled", "cancelled_by_user")
            else:
                await self._finalize(run_id, "failed", err)
        finally:
            await self.allocator.release(run_id)

    async def _clear_stop_error(self, run_id: int) -> None:
        async with self.session_factory() as session:
            run = await session.get(BotRun, run_id)
            if run and run.error == "stop_requested":
                run.error = None
            await session.commit()

    async def _update_status(
        self, run_id: int, status: str, *, slot: object | None = None
    ) -> None:
        async with self.session_factory() as session:
            run = await session.get(BotRun, run_id)
            if run is None or run.status in TERMINAL_BOT_STATUSES:
                await session.commit()
                return
            run.status = status
            if status == "joining" and run.started_at is None:
                run.started_at = _utcnow()
            if slot is not None:
                run.display = getattr(slot, "display", None)
                run.chrome_profile_path = str(getattr(slot, "profile_dir", ""))
            await session.commit()

    async def _upload_and_complete(
        self, run_id: int, user_id: str, raw_path: Path, staging: Path
    ) -> None:
        if not self.settings.s3_bucket:
            # Local-only success for dev without S3
            key = f"{self.settings.s3_prefix}/{user_id}/{run_id}/raw.webm"
            async with self.session_factory() as session:
                run = await session.get(BotRun, run_id)
                if run:
                    run.recording_path = str(raw_path)
                    run.status = "completed"
                    run.ended_at = _utcnow()
                    await enqueue_job(
                        session, kind="transcribe_recording", bot_run_id=run_id
                    )
                await session.commit()
            logger.warning("S3_BUCKET unset; kept local recording at %s", raw_path)
            return

        storage = S3Storage(self.settings)
        key = storage.recording_key(user_id, run_id, "raw.webm")
        await asyncio.to_thread(storage.upload_file, raw_path, key)

        audio = staging / "audio.webm"
        audio_key = None
        if audio.exists() and audio.stat().st_size > 0:
            try:
                audio_key = storage.recording_key(user_id, run_id, "audio.webm")
                await asyncio.to_thread(storage.upload_file, audio, audio_key)
            except Exception:
                logger.warning("Secondary audio upload failed run=%s", run_id)

        async with self.session_factory() as session:
            run = await session.get(BotRun, run_id)
            if run:
                run.recording_path = key
                run.audio_path = audio_key
                run.status = "completed"
                run.ended_at = _utcnow()
                await enqueue_job(session, kind="transcribe_recording", bot_run_id=run_id)
            await session.commit()

        if self.settings.meetops_delete_local_after_upload:
            shutil.rmtree(staging, ignore_errors=True)

    async def _finalize(self, run_id: int, status: str, error: str | None) -> None:
        async with self.session_factory() as session:
            run = await session.get(BotRun, run_id)
            if run and run.status not in TERMINAL_BOT_STATUSES:
                run.status = status
                run.error = error
                run.ended_at = _utcnow()
            await session.commit()