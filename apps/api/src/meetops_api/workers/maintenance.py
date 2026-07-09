"""Periodic maintenance workers."""

from __future__ import annotations

import logging
import shutil
from datetime import UTC, datetime, timedelta
from pathlib import Path

from sqlalchemy import delete, select
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from meetops_api.crypto.tokens import TokenCrypto
from meetops_api.google.calendar import CalendarSync
from meetops_common.models import AppKv, AuthEmailToken, CalendarConnection, Session
from meetops_common.settings import Settings

logger = logging.getLogger(__name__)


def _utcnow() -> datetime:
    return datetime.now(UTC)


async def session_cleanup(session_factory: async_sessionmaker[AsyncSession]) -> None:
    now = _utcnow()
    async with session_factory() as session:
        await session.execute(delete(Session).where(Session.expires_at <= now))
        await session.execute(delete(AuthEmailToken).where(AuthEmailToken.expires_at <= now))
        # OAuth state keys older than 10 minutes
        rows = await session.scalars(select(AppKv).where(AppKv.key.like("oauth_state:%")))
        for kv in rows:
            if (now - kv.updated_at).total_seconds() > 600:
                await session.delete(kv)
        await session.commit()
    logger.info("Session cleanup complete")


async def calendar_sync_all(
    session_factory: async_sessionmaker[AsyncSession], settings: Settings
) -> None:
    crypto = TokenCrypto(settings)
    async with session_factory() as session:
        await CalendarSync(session, settings, crypto).sync_all()
        await session.commit()


async def staging_cleanup(settings: Settings) -> None:
    staging = settings.staging_dir
    if not staging.exists():
        return
    cutoff = _utcnow() - timedelta(days=settings.meetops_staging_ttl_days)
    for user_dir in staging.iterdir():
        if not user_dir.is_dir():
            continue
        for run_dir in user_dir.iterdir():
            if not run_dir.is_dir():
                continue
            try:
                mtime = datetime.fromtimestamp(run_dir.stat().st_mtime, tz=UTC)
                if mtime < cutoff:
                    shutil.rmtree(run_dir, ignore_errors=True)
                    logger.info("Deleted stale staging %s", run_dir)
            except OSError:
                pass


async def bot_session_health(
    session_factory: async_sessionmaker[AsyncSession], settings: Settings
) -> None:
    profiles = Path(
        settings.meetops_bot_profiles_dir or settings.meetops_data_dir / "chrome-profiles"
    )
    default = profiles / "default"
    slot0 = profiles / "slot-0"
    valid = (
        (default / "storage-state.json").exists()
        or (default / "Default").exists()
        or (slot0 / "Default").exists()
        or (slot0 / "storage-state.json").exists()
    )
    status = "valid" if valid else "expired"
    async with session_factory() as session:
        rows = await session.scalars(select(CalendarConnection))
        for conn in rows:
            conn.bot_session_status = status
            conn.bot_session_checked_at = _utcnow()
        await session.commit()
    if not valid:
        logger.warning("Bot Google session appears missing/expired — bootstrap required")