"""MeetOps CLI."""

from __future__ import annotations

import argparse
import asyncio
import sys


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(prog="meetops-cli", description="MeetOps operator CLI")
    sub = parser.add_subparsers(dest="cmd", required=True)

    db = sub.add_parser("db", help="Database commands")
    db_sub = db.add_subparsers(dest="db_cmd", required=True)
    db_sub.add_parser("reset", help="Drop and remigrate (development only)")
    db_sub.add_parser("migrate", help="Run alembic upgrade head")

    bot = sub.add_parser("bot", help="Bot profile commands")
    bot_sub = bot.add_subparsers(dest="bot_cmd", required=True)
    boot = bot_sub.add_parser("profile-bootstrap", help="Open browser to sign into Google")
    boot.add_argument("--profile", default="default")
    bot_sub.add_parser("profile-status", help="Check bot profile presence")
    join = bot_sub.add_parser("join", help="Join a Meet URL (no record)")
    join.add_argument("meet_url")
    rec = bot_sub.add_parser("record", help="Record a Meet URL")
    rec.add_argument("meet_url")

    args = parser.parse_args(argv)

    if args.cmd == "db":
        if args.db_cmd == "migrate":
            _migrate()
        elif args.db_cmd == "reset":
            _db_reset()
    elif args.cmd == "bot":
        if args.bot_cmd == "profile-bootstrap":
            asyncio.run(_bootstrap(args.profile))
        elif args.bot_cmd == "profile-status":
            _profile_status()
        elif args.bot_cmd == "join":
            asyncio.run(_join(args.meet_url, record=False))
        elif args.bot_cmd == "record":
            asyncio.run(_join(args.meet_url, record=True))


def _migrate() -> None:
    from alembic import command
    from alembic.config import Config

    cfg = Config("alembic.ini")
    command.upgrade(cfg, "head")
    print("Migrations applied.")


def _db_reset() -> None:
    from meetops_common.settings import get_settings

    settings = get_settings()
    if settings.meetops_env != "development":
        print("db reset only allowed when MEETOPS_ENV=development", file=sys.stderr)
        sys.exit(1)
    import asyncio

    from sqlalchemy import text

    from meetops_common.db import get_engine

    async def _drop() -> None:
        engine = get_engine(settings)
        async with engine.begin() as conn:
            await conn.execute(text("DROP SCHEMA public CASCADE"))
            await conn.execute(text("CREATE SCHEMA public"))

    asyncio.run(_drop())
    _migrate()
    print("Database reset complete.")


async def _bootstrap(profile: str) -> None:
    from pathlib import Path

    from meetops_api.bot.browser import BrowserSession
    from meetops_api.bot.slots import BotSlot
    from meetops_common.settings import get_settings

    settings = get_settings()
    profiles = Path(settings.meetops_bot_profiles_dir or settings.meetops_data_dir / "chrome-profiles")
    slot = BotSlot(index=0, display=settings.meetops_bot_x11_display or ":99", profile_dir=profiles / profile)
    slot.profile_dir.mkdir(parents=True, exist_ok=True)
    slot.start_display()
    session = BrowserSession(settings, slot)
    try:
        await session.start()
        await session.goto("https://accounts.google.com/")
        print("Sign into Google in the browser/noVNC window, then press Enter here…")
        await asyncio.to_thread(input)
        await session.stop()
        print(f"Profile saved to {slot.profile_dir}")
    finally:
        try:
            await session.stop()
        except Exception:
            pass
        slot.stop_display()


def _profile_status() -> None:
    from pathlib import Path

    from meetops_common.settings import get_settings

    settings = get_settings()
    profiles = Path(settings.meetops_bot_profiles_dir or settings.meetops_data_dir / "chrome-profiles")
    for name in ("default", "slot-0"):
        p = profiles / name
        ok = (p / "Default").exists() or (p / "storage-state.json").exists()
        print(f"{name}: {'ok' if ok else 'missing'} ({p})")


async def _join(meet_url: str, *, record: bool) -> None:
    import asyncio
    from pathlib import Path

    from meetops_api.bot.recorder import MeetRecorder
    from meetops_api.bot.slots import BotSlot
    from meetops_api.services.meet_url import normalize_meet_url
    from meetops_common.db import get_session_factory
    from meetops_common.settings import get_settings

    settings = get_settings()
    url = normalize_meet_url(meet_url)
    profiles = Path(settings.meetops_bot_profiles_dir or settings.meetops_data_dir / "chrome-profiles")
    slot = BotSlot(index=0, display=settings.meetops_bot_x11_display or ":99", profile_dir=profiles / "slot-0")
    slot.start_display()
    stop = asyncio.Event()
    staging = settings.staging_dir / "cli" / "adhoc"
    if not record:
        from meetops_api.bot.browser import BrowserSession

        session = BrowserSession(settings, slot)
        try:
            await session.start()
            await session.goto(url)
            print("Joined (manual). Press Enter to close…")
            await asyncio.to_thread(input)
        finally:
            await session.stop()
            slot.stop_display()
        return

    recorder = MeetRecorder(
        settings,
        slot,
        session_factory=get_session_factory(settings),
        bot_run_id=0,
        meet_url=url,
        display_name=settings.meetops_bot_display_name,
        stop_event=stop,
        staging_dir=staging,
        max_duration_secs=settings.meetops_bot_adhoc_duration_secs,
    )
    try:
        path = await recorder.run()
        print(f"Recording saved: {path}")
    finally:
        slot.stop_display()


if __name__ == "__main__":
    main()