"""Patchright browser session for Meet recording."""

from __future__ import annotations

import logging
import os
from pathlib import Path
from typing import Any

from meetops_api.bot.slots import BotSlot, clear_singleton_locks, ensure_profile_seed
from meetops_common.settings import Settings

logger = logging.getLogger(__name__)

SCRIPTS_DIR = Path(__file__).parent / "scripts"


def load_script(name: str) -> str:
    return (SCRIPTS_DIR / name).read_text(encoding="utf-8")


def chrome_args(settings: Settings) -> list[str]:
    w, h = settings.meetops_bot_window_width, settings.meetops_bot_window_height
    args = [
        "--no-sandbox",
        "--disable-dev-shm-usage",
        f"--window-size={w},{h}",
        "--start-maximized",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-session-crashed-bubble",
        "--hide-crash-restore-bubble",
        "--disable-infobars",
        "--test-type",
        f"--lang={settings.meetops_bot_locale}",
        f"--accept-lang={settings.meetops_bot_locale},en",
    ]
    if settings.meetops_use_tab_capture_auto_accept:
        args.append("--auto-accept-this-tab-capture")
        logger.warning(
            "Tab-capture auto-accept flag enabled — increases bot detectability"
        )
    return args


class BrowserSession:
    def __init__(self, settings: Settings, slot: BotSlot) -> None:
        self.settings = settings
        self.slot = slot
        self._pw: Any = None
        self._context: Any = None
        self._page: Any = None

    @property
    def page(self) -> Any:
        return self._page

    @property
    def context(self) -> Any:
        return self._context

    async def start(self) -> None:
        from patchright.async_api import async_playwright

        seed = (
            Path(
                self.settings.meetops_bot_profiles_dir
                or self.settings.meetops_data_dir / "chrome-profiles"
            )
            / "default"
        )
        ensure_profile_seed(self.slot.profile_dir, seed)
        clear_singleton_locks(self.slot.profile_dir)

        # Per-launch env only: concurrent slots must not share a process-wide DISPLAY.
        env = {**os.environ, "DISPLAY": self.slot.display}

        self._pw = await async_playwright().start()
        proxy = (
            {"server": self.settings.meetops_browser_proxy}
            if self.settings.meetops_browser_proxy
            else None
        )
        launch_kwargs: dict[str, Any] = {
            "user_data_dir": str(self.slot.profile_dir),
            "headless": False,
            "args": chrome_args(self.settings),
            "viewport": None,
            "no_viewport": True,
            "env": env,
            "locale": self.settings.meetops_bot_locale,  # Accept-Language + navigator.language
        }
        if proxy:
            launch_kwargs["proxy"] = proxy
        self._context = await self._pw.chromium.launch_persistent_context(**launch_kwargs)
        self._page = (
            self._context.pages[0] if self._context.pages else await self._context.new_page()
        )

    async def stop(self) -> None:
        try:
            if self._context:
                try:
                    state_path = self.slot.profile_dir / "storage-state.json"
                    await self._context.storage_state(path=str(state_path))
                except Exception:
                    logger.debug("Failed to save storage_state", exc_info=True)
                await self._context.close()
        finally:
            self._context = None
            self._page = None
            if self._pw:
                await self._pw.stop()
                self._pw = None

    async def goto(self, url: str) -> None:
        assert self._page
        await self._page.goto(url, wait_until="domcontentloaded", timeout=60_000)