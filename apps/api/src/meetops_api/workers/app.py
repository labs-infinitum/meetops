"""Worker process entry helpers shared with meetops_worker."""

from __future__ import annotations

import asyncio
import logging
import signal
from typing import Any

from meetops_api.bot.slots import SlotAllocator
from meetops_api.logging import setup_logging
from meetops_api.workers.ai_pipeline import AIPipeline
from meetops_api.workers.bot_runner import BotRunner
from meetops_api.workers.maintenance import (
    bot_session_health,
    calendar_sync_all,
    session_cleanup,
    staging_cleanup,
)
from meetops_api.workers.scheduler import Scheduler
from meetops_common.db import dispose_engine, get_session_factory
from meetops_common.settings import get_settings

logger = logging.getLogger(__name__)


class WorkerApp:
    def __init__(self) -> None:
        self.settings = get_settings()
        setup_logging(self.settings)
        self.session_factory = get_session_factory(self.settings)
        self.allocator = SlotAllocator(self.settings)
        self.bot_runner = BotRunner(self.settings, self.session_factory, self.allocator)
        self.scheduler = Scheduler(self.settings, self.session_factory, self.bot_runner)
        self.ai = AIPipeline(self.settings, self.session_factory)
        self._tasks: list[asyncio.Task[Any]] = []
        self._shutdown = asyncio.Event()

    async def start(self) -> None:
        await self.bot_runner.reconcile_abandoned()
        # Immediate calendar sync on startup
        try:
            await calendar_sync_all(self.session_factory, self.settings)
        except Exception:
            logger.exception("Startup calendar sync failed")

        self._tasks = [
            asyncio.create_task(self._loop_scheduler(), name="scheduler"),
            asyncio.create_task(self._loop_ai(), name="ai"),
            asyncio.create_task(self._loop_calendar(), name="calendar"),
            asyncio.create_task(self._loop_session_cleanup(), name="session_cleanup"),
            asyncio.create_task(self._loop_staging(), name="staging"),
            asyncio.create_task(self._loop_bot_health(), name="bot_health"),
        ]
        logger.info(
            "Worker started concurrency=%s", self.settings.meetops_bot_concurrency
        )
        await self._shutdown.wait()

    async def shutdown(self) -> None:
        logger.info("Worker shutting down…")
        self.scheduler.stop_accepting()
        self.bot_runner.stop_accepting()
        await self.bot_runner.signal_all_stop()
        await self.bot_runner.wait_active(timeout=120)
        for t in self._tasks:
            t.cancel()
        await asyncio.gather(*self._tasks, return_exceptions=True)
        await dispose_engine()
        self._shutdown.set()

    async def _loop_scheduler(self) -> None:
        while True:
            try:
                await self.scheduler.tick()
            except Exception:
                logger.exception("Scheduler tick failed")
            await asyncio.sleep(self.settings.meetops_bot_scheduler_poll_secs)

    async def _loop_ai(self) -> None:
        while True:
            try:
                await self.ai.tick()
            except Exception:
                logger.exception("AI tick failed")
            await asyncio.sleep(5)

    async def _loop_calendar(self) -> None:
        while True:
            await asyncio.sleep(self.settings.meetops_calendar_sync_interval_secs)
            try:
                await calendar_sync_all(self.session_factory, self.settings)
            except Exception:
                logger.exception("Calendar sync failed")

    async def _loop_session_cleanup(self) -> None:
        while True:
            try:
                await session_cleanup(self.session_factory)
            except Exception:
                logger.exception("Session cleanup failed")
            await asyncio.sleep(3600)

    async def _loop_staging(self) -> None:
        while True:
            try:
                await staging_cleanup(self.settings)
            except Exception:
                logger.exception("Staging cleanup failed")
            await asyncio.sleep(86400)

    async def _loop_bot_health(self) -> None:
        while True:
            try:
                await bot_session_health(self.session_factory, self.settings)
            except Exception:
                logger.exception("Bot health check failed")
            await asyncio.sleep(900)


async def run_worker() -> None:
    app = WorkerApp()
    loop = asyncio.get_running_loop()

    def _signal_handler() -> None:
        asyncio.create_task(app.shutdown())

    for sig in (signal.SIGTERM, signal.SIGINT):
        try:
            loop.add_signal_handler(sig, _signal_handler)
        except NotImplementedError:
            signal.signal(sig, lambda *_: asyncio.create_task(app.shutdown()))

    await app.start()


def run() -> None:
    asyncio.run(run_worker())