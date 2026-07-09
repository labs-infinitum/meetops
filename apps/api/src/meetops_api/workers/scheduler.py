"""Scheduler: claim eligible meetings and ad-hoc queued runs."""

from __future__ import annotations

import logging
from datetime import UTC, datetime, timedelta

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from meetops_api.workers.bot_runner import BotRunner
from meetops_common.models import BotRun, Meeting, UserSettings
from meetops_common.settings import Settings

logger = logging.getLogger(__name__)


def _utcnow() -> datetime:
    return datetime.now(UTC)


class Scheduler:
    def __init__(
        self,
        settings: Settings,
        session_factory: async_sessionmaker[AsyncSession],
        bot_runner: BotRunner,
    ) -> None:
        self.settings = settings
        self.session_factory = session_factory
        self.bot_runner = bot_runner
        self._accepting = True

    def stop_accepting(self) -> None:
        self._accepting = False

    async def tick(self) -> None:
        if not self._accepting:
            return
        await self._claim_scheduled()
        await self._claim_adhoc()

    async def _claim_scheduled(self) -> None:
        now = _utcnow()
        async with self.session_factory() as session:
            meetings = await session.scalars(
                select(Meeting).where(
                    Meeting.auto_join.is_(True),
                    Meeting.meet_url.is_not(None),
                    Meeting.end_at > now,
                )
            )
            for meeting in meetings:
                settings_row = await session.get(UserSettings, meeting.user_id)
                lead = (
                    settings_row.join_lead_time_secs
                    if settings_row
                    else self.settings.meetops_bot_join_lead_time_secs
                )
                join_at = meeting.start_at - timedelta(seconds=lead)
                if not (join_at <= now < meeting.end_at):
                    continue
                existing = await session.scalar(
                    select(BotRun.id).where(
                        BotRun.meeting_id == meeting.id,
                        BotRun.meeting_start_at == meeting.start_at,
                    )
                )
                if existing:
                    continue
                run = BotRun(
                    meeting_id=meeting.id,
                    meeting_start_at=meeting.start_at,
                    user_id=meeting.user_id,
                    status="queued",
                    trigger="scheduled",
                    meet_url=meeting.meet_url,
                )
                session.add(run)
                await session.flush()
                run_id = run.id
                await session.commit()
                logger.info("Scheduled bot run=%s meeting=%s", run_id, meeting.id)
                await self.bot_runner.spawn_if_needed(run_id)
                return  # one per tick to avoid stampede; next tick continues

    async def _claim_adhoc(self) -> None:
        async with self.session_factory() as session:
            rows = await session.scalars(
                select(BotRun)
                .where(BotRun.status == "queued", BotRun.trigger == "adhoc")
                .order_by(BotRun.created_at.asc())
                .limit(self.settings.meetops_bot_concurrency)
            )
            ids = [r.id for r in rows]
            await session.commit()
        for run_id in ids:
            await self.bot_runner.spawn_if_needed(run_id)

        # Also spawn queued scheduled that were created but not yet running
        async with self.session_factory() as session:
            rows = await session.scalars(
                select(BotRun)
                .where(BotRun.status == "queued", BotRun.trigger == "scheduled")
                .order_by(BotRun.created_at.asc())
                .limit(self.settings.meetops_bot_concurrency)
            )
            ids = [r.id for r in rows]
            await session.commit()
        for run_id in ids:
            await self.bot_runner.spawn_if_needed(run_id)