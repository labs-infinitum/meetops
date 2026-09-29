"""Display/slot allocator for concurrent bot recordings."""

from __future__ import annotations

import asyncio
import logging
import os
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

from meetops_common.settings import Settings

logger = logging.getLogger(__name__)


def slot_displays(configured: str | None, count: int) -> list[tuple[str, bool]]:
    """X display per slot as (display, owned).

    Every slot needs its own display: xdotool (Allow sheet, join fallback) targets
    "the Chrome window" on a display, so two bots sharing one would click each
    other's windows. The configured display (started by the container entrypoint)
    goes to slot 0; later slots get the following display numbers and start their
    own Xvfb (owned=True).
    """
    if configured:
        base = int(configured.lstrip(":").split(".")[0])
        return [(configured, False)] + [(f":{base + i}", True) for i in range(1, count)]
    return [(f":{10 + i}", True) for i in range(count)]


@dataclass
class BotSlot:
    index: int
    display: str
    profile_dir: Path
    owns_display: bool = True
    _xvfb_proc: subprocess.Popen[bytes] | None = None
    _openbox_proc: subprocess.Popen[bytes] | None = None

    def start_display(self) -> None:
        if not self.owns_display:
            return
        try:
            self._xvfb_proc = subprocess.Popen(
                [
                    "Xvfb",
                    self.display,
                    "-screen",
                    "0",
                    "1920x1080x24",
                    "-ac",
                    "+extension",
                    "RANDR",
                ],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            self._wait_for_x_socket()
            self._openbox_proc = subprocess.Popen(
                ["openbox"],
                env={**os.environ, "DISPLAY": self.display},
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        except FileNotFoundError:
            logger.warning("Xvfb/openbox not available; using existing DISPLAY")

    def _wait_for_x_socket(self, timeout: float = 5.0) -> None:
        socket_path = Path(f"/tmp/.X11-unix/X{self.display.lstrip(':').split('.')[0]}")
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if socket_path.exists():
                return
            if self._xvfb_proc and self._xvfb_proc.poll() is not None:
                logger.warning("Xvfb exited early for display %s", self.display)
                return
            time.sleep(0.05)
        logger.warning("Xvfb socket for display %s not ready after %.0fs", self.display, timeout)

    def stop_display(self) -> None:
        for proc in (self._openbox_proc, self._xvfb_proc):
            if proc and proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    proc.kill()
        self._openbox_proc = None
        self._xvfb_proc = None


class SlotAllocator:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self._lock = asyncio.Lock()
        n = max(1, settings.meetops_bot_concurrency)
        profiles = Path(settings.meetops_bot_profiles_dir or settings.meetops_data_dir / "chrome-profiles")
        profiles.mkdir(parents=True, exist_ok=True)
        self._free: list[BotSlot] = []
        for i, (display, owned) in enumerate(slot_displays(settings.meetops_bot_x11_display, n)):
            slot = BotSlot(
                index=i,
                display=display,
                profile_dir=profiles / f"slot-{i}",
                owns_display=owned,
            )
            slot.profile_dir.mkdir(parents=True, exist_ok=True)
            self._free.append(slot)
        self._active: dict[int, BotSlot] = {}

    @property
    def available(self) -> int:
        return len(self._free)

    async def acquire(self, run_id: int) -> BotSlot:
        while True:
            async with self._lock:
                if self._free:
                    slot = self._free.pop(0)
                    self._active[run_id] = slot
                    slot.start_display()
                    return slot
            await asyncio.sleep(0.5)

    async def release(self, run_id: int) -> None:
        async with self._lock:
            slot = self._active.pop(run_id, None)
            if slot:
                slot.stop_display()
                self._free.append(slot)


def clear_singleton_locks(profile_dir: Path) -> None:
    """Clear stale Chromium singleton locks only when no live Chrome PID holds them."""
    for name in ("SingletonLock", "SingletonSocket", "SingletonCookie"):
        path = profile_dir / name
        if not path.exists() and not path.is_symlink():
            continue
        # If symlink points to a live PID, skip
        try:
            if path.is_symlink():
                target = os.readlink(path)
                # format often hostname-pid
                if "-" in target:
                    pid_s = target.rsplit("-", 1)[-1]
                    if pid_s.isdigit() and _pid_alive(int(pid_s)):
                        logger.info("Skipping lock clear; Chrome pid %s alive", pid_s)
                        continue
            path.unlink(missing_ok=True)
        except OSError:
            logger.warning("Failed to clear %s", path)


def _pid_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def ensure_profile_seed(slot_profile: Path, seed_dir: Path) -> None:
    """Copy seed profile into slot if slot is empty and seed exists."""
    if (slot_profile / "Default").exists():
        return
    if not (seed_dir / "Default").exists() and not (seed_dir / "storage-state.json").exists():
        return
    if seed_dir.resolve() == slot_profile.resolve():
        return
    try:
        if seed_dir.exists():
            for item in seed_dir.iterdir():
                dest = slot_profile / item.name
                if dest.exists():
                    continue
                if item.is_dir():
                    shutil.copytree(item, dest, dirs_exist_ok=True)
                else:
                    shutil.copy2(item, dest)
    except OSError:
        logger.exception("Failed to seed profile from %s", seed_dir)