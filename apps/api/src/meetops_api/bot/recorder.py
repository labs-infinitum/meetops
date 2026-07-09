"""Meet join + tab recording orchestration."""

from __future__ import annotations

import asyncio
import base64
import json
import logging
import os
import subprocess
from collections.abc import Awaitable, Callable
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from meetops_api.bot.browser import BrowserSession, load_script
from meetops_api.bot.slots import BotSlot
from meetops_common.models import MeetingTimelineEvent
from meetops_common.settings import Settings

logger = logging.getLogger(__name__)

MIN_WEBM_BYTES = 1024


def _utcnow() -> datetime:
    return datetime.now(UTC)


class MeetRecorder:
    def __init__(
        self,
        settings: Settings,
        slot: BotSlot,
        *,
        session_factory: async_sessionmaker[AsyncSession],
        bot_run_id: int,
        meet_url: str,
        display_name: str,
        stop_event: asyncio.Event,
        staging_dir: Path,
        on_status: Callable[[str], Awaitable[None]] | None = None,
        end_at: datetime | None = None,
        max_duration_secs: int | None = None,
    ) -> None:
        self.settings = settings
        self.slot = slot
        self.session_factory = session_factory
        self.bot_run_id = bot_run_id
        self.meet_url = meet_url
        self.display_name = display_name
        self.stop_event = stop_event
        self.staging_dir = staging_dir
        self.on_status = on_status
        self.end_at = end_at
        self.max_duration_secs = max_duration_secs
        self.browser = BrowserSession(settings, slot)
        self.raw_path = staging_dir / "raw.webm"
        self.audio_path = staging_dir / "audio.webm"
        self._chunk_file = None
        self._audio_file = None
        self._started_at: datetime | None = None
        self._solo_since: datetime | None = None

    async def _set_status(self, status: str) -> None:
        if self.on_status:
            await self.on_status(status)

    async def run(self) -> Path:
        self.staging_dir.mkdir(parents=True, exist_ok=True)
        self._chunk_file = self.raw_path.open("ab")
        self._audio_file = self.audio_path.open("ab")
        try:
            await self.browser.start()
            await self._set_status("joining")
            await self.browser.goto(self.meet_url)
            joined = await self._join_loop()
            if not joined:
                raise RuntimeError("Failed to join meeting (denied or admission timeout)")
            await self._set_status("in_meeting")
            await self._dismiss_overlays()
            await self._prime_roster()
            await self._grant_permissions()
            await self._install_and_start_recording()
            await self._set_status("recording")
            self._started_at = _utcnow()
            await self._recording_loop()
            await self._stop_recording()
        finally:
            try:
                await self.browser.stop()
            except Exception:
                logger.exception("browser stop failed run=%s", self.bot_run_id)
            for attr in ("_chunk_file", "_audio_file"):
                fh = getattr(self, attr, None)
                if fh:
                    try:
                        fh.flush()
                        os.fsync(fh.fileno())
                        fh.close()
                    except Exception:
                        pass
                    setattr(self, attr, None)

        if not self.raw_path.exists() or self.raw_path.stat().st_size < MIN_WEBM_BYTES:
            raise RuntimeError("Recording file missing or too small")
        # If we have a separate audio track file, remux into raw.webm when possible
        await self._maybe_mux_audio()
        return self.raw_path

    async def _eval_script(self, name: str, *, display_name: str | None = None) -> Any:
        src = load_script(name)
        if display_name is not None:
            src = src.replace("__DISPLAY_NAME_JSON__", json.dumps(display_name))
        page = self.browser.page
        return await page.evaluate(f"({src})()")

    async def _stop_requested(self) -> bool:
        """True if local stop Event is set OR DB row has stop_requested (cross-process)."""
        if self.stop_event.is_set():
            return True
        try:
            from meetops_common.models import BotRun

            async with self.session_factory() as session:
                run = await session.get(BotRun, self.bot_run_id)
                await session.commit()
                if run is None:
                    return False
                if run.error == "stop_requested":
                    self.stop_event.set()
                    return True
                if run.status in ("cancelled", "failed", "completed"):
                    self.stop_event.set()
                    return True
        except Exception:
            logger.debug("stop poll failed run=%s", self.bot_run_id, exc_info=True)
        return False

    async def _join_loop(self) -> bool:
        deadline = asyncio.get_event_loop().time() + self.settings.meetops_bot_admission_timeout_secs
        while asyncio.get_event_loop().time() < deadline:
            if await self._stop_requested():
                raise RuntimeError("cancelled_by_user")
            # Dismiss "Got it" / sign-in tips before each join attempt
            await self._dismiss_overlays()
            await self._click_prejoin_fallbacks()
            try:
                result = await self._eval_script(
                    "attempt_join.js", display_name=self.display_name
                )
            except Exception:
                logger.warning("attempt_join error", exc_info=True)
                result = {"state": "needs_action"}
            state = (result or {}).get("state") if isinstance(result, dict) else None
            message = (result or {}).get("message") if isinstance(result, dict) else None
            logger.info(
                "join poll run=%s state=%s message=%s",
                self.bot_run_id,
                state,
                message,
            )
            if state == "joined":
                return True
            if state == "denied":
                return False
            # If still on pre-join, force a real pointer click on Ask to join
            if state in ("needs_action", None):
                await self._click_ask_to_join()
            await asyncio.sleep(2)
        return False

    async def _click_prejoin_fallbacks(self) -> None:
        """Playwright clicks for overlays that ignore DOM .click()."""
        page = self.browser.page
        for label in ("Got it", "Continue without microphone and camera", "Dismiss"):
            try:
                loc = page.get_by_role("button", name=label)
                if await loc.count() > 0 and await loc.first.is_visible():
                    await loc.first.click(timeout=2000)
                    logger.info("Clicked prejoin control %r run=%s", label, self.bot_run_id)
                    await asyncio.sleep(0.4)
            except Exception:
                logger.debug("prejoin fallback %r failed", label, exc_info=True)

    async def _click_ask_to_join(self) -> None:
        page = self.browser.page
        for label in ("Ask to join", "Join now", "Join"):
            try:
                loc = page.get_by_role("button", name=label)
                if await loc.count() == 0:
                    loc = page.get_by_text(label, exact=True)
                if await loc.count() > 0 and await loc.first.is_visible():
                    await loc.first.click(timeout=3000)
                    logger.info("Clicked %r via Playwright run=%s", label, self.bot_run_id)
                    await asyncio.sleep(0.5)
                    return
            except Exception:
                logger.debug("Playwright click %r failed", label, exc_info=True)
        # Last resort: xdotool click near typical Ask-to-join position
        await self._xdotool_click_ask_to_join()

    async def _xdotool_click_ask_to_join(self) -> None:
        display = self.slot.display
        env = {**os.environ, "DISPLAY": display}
        # Approximate center-right of 1280x720 prejoin (Ask to join button)
        for x, y in ((980, 420), (1000, 450), (920, 400)):
            try:
                subprocess.run(
                    ["xdotool", "mousemove", str(x), str(y), "click", "1"],
                    env=env,
                    timeout=2,
                    check=False,
                    capture_output=True,
                )
            except (FileNotFoundError, subprocess.TimeoutExpired):
                return

    async def _dismiss_overlays(self) -> None:
        try:
            await self._eval_script("dismiss_meet_overlays.js")
        except Exception:
            logger.debug("dismiss overlays failed", exc_info=True)

    async def _prime_roster(self) -> None:
        try:
            await self._eval_script("prime_participant_roster.js")
        except Exception:
            logger.debug("prime roster failed", exc_info=True)

    async def _grant_permissions(self) -> None:
        try:
            cdp = await self.browser.context.new_cdp_session(self.browser.page)
            await cdp.send(
                "Browser.grantPermissions",
                {
                    "origin": "https://meet.google.com",
                    "permissions": [
                        "audioCapture",
                        "videoCapture",
                        "displayCapture",
                    ],
                },
            )
        except Exception:
            logger.debug("CDP grantPermissions failed", exc_info=True)

    async def _install_and_start_recording(self) -> None:
        page = self.browser.page
        # Expose binding for chunks
        await page.expose_binding(
            "__gmrRecordChunk",
            lambda _source, payload: self._on_chunk(payload),
        )
        await self._eval_script("install_recording_control.js")

        # Start Allow-clicker BEFORE triggering getDisplayMedia so we catch the sheet
        allow_task = asyncio.create_task(
            self._accept_tab_capture_loop(),
            name=f"tab-capture-allow-{self.bot_run_id}",
        )
        try:
            try:
                await page.click("#gmr-start-recording", timeout=5000)
            except Exception:
                logger.debug("start recording button click failed; trying evaluate")
                await page.evaluate(
                    """() => {
                      const btn = document.querySelector('#gmr-start-recording');
                      if (btn) btn.click();
                    }"""
                )
            # Give the native sheet time to appear, then keep clicking Allow
            await asyncio.sleep(1.5)
            await asyncio.wait_for(allow_task, timeout=20)
        except TimeoutError:
            logger.warning(
                "tab-capture Allow loop timed out run=%s — click Allow in noVNC if still visible",
                self.bot_run_id,
            )
            allow_task.cancel()
        except Exception:
            allow_task.cancel()
            raise

    async def _accept_tab_capture_loop(self) -> None:
        from meetops_api.bot.tab_capture_allow import accept_tab_capture_loop

        await accept_tab_capture_loop(
            self.slot.display,
            attempts=15,
            interval_secs=0.8,
            should_abort=self._stop_requested,
        )

    def _on_chunk(self, payload: Any) -> None:
        try:
            if isinstance(payload, str):
                payload = json.loads(payload)
            if not isinstance(payload, dict):
                return
            kind = payload.get("kind")
            if kind in ("chunk", "audio_chunk"):
                data_b64 = payload.get("data") or payload.get("base64")
                target = self._audio_file if kind == "audio_chunk" else self._chunk_file
                if data_b64 and target:
                    try:
                        raw = base64.b64decode(data_b64)
                        target.write(raw)
                        if target.tell() % (256 * 1024) < len(raw):
                            target.flush()
                    except Exception:
                        logger.warning("Malformed recording chunk (%s); continuing", kind)
            elif kind == "timeline":
                events = payload.get("events") or []
                asyncio.create_task(self._ingest_timeline(events))
            elif kind == "done":
                for fh in (self._chunk_file, getattr(self, "_audio_file", None)):
                    if fh:
                        fh.flush()
            elif kind == "status":
                logger.info(
                    "recording status run=%s audioTracks=%s source=%s",
                    self.bot_run_id,
                    payload.get("audioTrackCount"),
                    payload.get("audioSource"),
                )
        except Exception:
            logger.warning("Chunk handler error; continuing", exc_info=True)

    async def _maybe_mux_audio(self) -> None:
        """If audio.webm has data and raw.webm may be silent, remux with ffmpeg."""
        if not self.audio_path.exists() or self.audio_path.stat().st_size < 256:
            logger.warning(
                "No separate audio track for run=%s (size=%s) — video may be silent",
                self.bot_run_id,
                self.audio_path.stat().st_size if self.audio_path.exists() else 0,
            )
            return
        out = self.staging_dir / "muxed.webm"
        try:
            proc = await asyncio.create_subprocess_exec(
                "ffmpeg",
                "-y",
                "-i",
                str(self.raw_path),
                "-i",
                str(self.audio_path),
                "-c:v",
                "copy",
                "-c:a",
                "copy",
                "-shortest",
                str(out),
                stdout=asyncio.subprocess.DEVNULL,
                stderr=asyncio.subprocess.PIPE,
            )
            _, err = await proc.communicate()
            if proc.returncode == 0 and out.exists() and out.stat().st_size > MIN_WEBM_BYTES:
                out.replace(self.raw_path)
                logger.info("Muxed audio into raw.webm for run=%s", self.bot_run_id)
            else:
                logger.warning(
                    "ffmpeg mux failed run=%s code=%s: %s",
                    self.bot_run_id,
                    proc.returncode,
                    (err or b"")[:500].decode(errors="ignore"),
                )
        except FileNotFoundError:
            logger.warning("ffmpeg not available; leaving video/audio as separate files")
        except Exception:
            logger.exception("audio mux failed run=%s", self.bot_run_id)

    async def _ingest_timeline(self, events: list[dict[str, Any]]) -> None:
        if not events:
            return
        async with self.session_factory() as session:
            try:
                for ev in events:
                    session.add(
                        MeetingTimelineEvent(
                            bot_run_id=self.bot_run_id,
                            event_type=ev.get("type") or ev.get("event_type") or "collector_warning",
                            occurred_at=_utcnow(),
                            relative_ms=ev.get("relative_ms") or ev.get("t"),
                            participant_id=ev.get("participant_id"),
                            participant_name=ev.get("participant_name") or ev.get("name"),
                            payload_json=ev,
                        )
                    )
                await session.commit()
            except Exception:
                await session.rollback()
                logger.exception("timeline ingest failed")

    async def _xdotool_click_ask_to_join(self) -> None:
        display = self.slot.display
        env = {**os.environ, "DISPLAY": display}
        # Approximate center-right of 1280x720 prejoin (Ask to join button)
        for x, y in ((980, 420), (1000, 450), (920, 400)):
            try:
                subprocess.run(
                    ["xdotool", "mousemove", str(x), str(y), "click", "1"],
                    env=env,
                    timeout=2,
                    check=False,
                    capture_output=True,
                )
            except (FileNotFoundError, subprocess.TimeoutExpired):
                return

    async def _recording_loop(self) -> None:
        while True:
            if await self._stop_requested():
                logger.info("Stop requested for run=%s — leaving recording loop", self.bot_run_id)
                break
            now = _utcnow()
            if self.end_at and now >= self.end_at:
                logger.info("Meeting end_at reached run=%s", self.bot_run_id)
                break
            if self.max_duration_secs and self._started_at:
                if (now - self._started_at).total_seconds() >= self.max_duration_secs:
                    logger.info("Ad-hoc max duration reached run=%s", self.bot_run_id)
                    break
            try:
                await self._dismiss_overlays()
                count = await self._eval_script("meet_participant_count.js")
                n = (
                    int(count)
                    if isinstance(count, (int, float))
                    else (int(count.get("count", 1)) if isinstance(count, dict) else 1)
                )
                if n <= 1:
                    if self._solo_since is None:
                        self._solo_since = now
                    elif (
                        now - self._solo_since
                    ).total_seconds() >= self.settings.meetops_bot_solo_leave_secs:
                        logger.info("Solo leave timeout run=%s", self.bot_run_id)
                        break
                else:
                    self._solo_since = None
            except Exception:
                logger.debug("recording loop probe failed", exc_info=True)
            # Short sleep; stop is polled via DB each iteration
            try:
                await asyncio.wait_for(self.stop_event.wait(), timeout=2)
            except TimeoutError:
                pass

    async def _stop_recording(self) -> None:
        try:
            await self._eval_script("stop_tab_recording.js")
            await asyncio.sleep(2)
        except Exception:
            logger.warning("stop_tab_recording failed", exc_info=True)
        # Leave the Meet call so the bot disappears from the roster
        try:
            page = self.browser.page
            for label in ("Leave call", "Leave", "End call"):
                loc = page.get_by_role("button", name=label)
                if await loc.count() > 0:
                    await loc.first.click(timeout=3000)
                    logger.info("Clicked %r to leave Meet run=%s", label, self.bot_run_id)
                    break
        except Exception:
            logger.debug("leave-call click failed", exc_info=True)