"""Accept Chrome's native \"Allow meet.google.com to see this tab?\" sheet via xdotool."""

from __future__ import annotations

import asyncio
import logging
import os
import subprocess

logger = logging.getLogger(__name__)


def _run(args: list[str], display: str, timeout: float = 5.0) -> subprocess.CompletedProcess[bytes]:
    env = {**os.environ, "DISPLAY": display}
    return subprocess.run(args, env=env, timeout=timeout, check=False, capture_output=True)


def _find_chrome_window(display: str) -> str | None:
    for pattern in ("Chromium", "Chrome", "Google Chrome", "chromium"):
        try:
            out = _run(
                ["xdotool", "search", "--onlyvisible", "--class", pattern],
                display,
            )
            if out.returncode == 0 and out.stdout.strip():
                # Prefer the last (often topmost / largest) id
                ids = out.stdout.decode().strip().split()
                if ids:
                    return ids[-1]
        except (FileNotFoundError, subprocess.TimeoutExpired):
            return None
    # Fallback: any window with "meet" in the title
    try:
        out = _run(["xdotool", "search", "--onlyvisible", "--name", "Meet"], display)
        if out.returncode == 0 and out.stdout.strip():
            return out.stdout.decode().strip().split()[-1]
    except (FileNotFoundError, subprocess.TimeoutExpired):
        pass
    return None


def _parse_geometry(stdout: str) -> tuple[int, int, int, int] | None:
    vals: dict[str, int] = {}
    for line in stdout.splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            try:
                vals[k.strip()] = int(v.strip())
            except ValueError:
                continue
    if all(k in vals for k in ("X", "Y", "WIDTH", "HEIGHT")):
        return vals["X"], vals["Y"], vals["WIDTH"], vals["HEIGHT"]
    return None


def _click_frac(
    display: str, wid: str, x: int, y: int, w: int, h: int, xf: float, yf: float
) -> None:
    cx = int(x + w * xf)
    cy = int(y + h * yf)
    _run(["xdotool", "mousemove", "--window", wid, str(int(w * xf)), str(int(h * yf))], display)
    # Also absolute screen coords (more reliable for some Chrome builds)
    _run(["xdotool", "mousemove", str(cx), str(cy)], display)
    _run(["xdotool", "click", "1"], display)


def accept_tab_capture_once(display: str) -> bool:
    """One pass: activate Chrome and click likely Allow positions. Returns True if tools ran."""
    try:
        which = _run(["which", "xdotool"], display, timeout=1.0)
        if which.returncode != 0:
            logger.warning("xdotool not found — click Allow manually in noVNC")
            return False
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False

    wid = _find_chrome_window(display)
    if not wid:
        logger.warning("No Chrome window found for tab-capture Allow click")
        return False

    _run(["xdotool", "windowactivate", "--sync", wid], display)
    _run(["xdotool", "windowfocus", wid], display)

    geo_out = _run(["xdotool", "getwindowgeometry", "--shell", wid], display)
    geo = _parse_geometry(geo_out.stdout.decode(errors="ignore")) if geo_out.returncode == 0 else None

    if geo:
        x, y, w, h = geo
        # Lower-right of the sheet (Allow sits right of Cancel).
        # Avoid mid-dialog "Also share tab audio" toggle — do NOT Tab then Return.
        for xf, yf in (
            (0.90, 0.88),
            (0.88, 0.91),
            (0.92, 0.87),
            (0.86, 0.93),
            (0.78, 0.72),
            (0.82, 0.70),
        ):
            try:
                _click_frac(display, wid, x, y, w, h, xf, yf)
            except (FileNotFoundError, subprocess.TimeoutExpired):
                return False

    # Prefer Right/Return (moves to Allow) — never bare Tab (focuses audio toggle)
    for spec in (["Right", "Return"], ["Right", "Right", "Return"]):
        try:
            _run(["xdotool", "key", "--window", wid, *spec], display)
            _run(["xdotool", "key", *spec], display)
        except (FileNotFoundError, subprocess.TimeoutExpired):
            break

    logger.info("tab-capture Allow xdotool pass finished (display=%s wid=%s)", display, wid)
    return True


async def accept_tab_capture_loop(
    display: str,
    *,
    attempts: int = 12,
    interval_secs: float = 1.0,
    should_abort: object | None = None,
) -> None:
    """Retry Allow acceptance while the sheet may still be opening.

    ``should_abort`` may be an async callable returning bool (e.g. stop requested).
    """
    for i in range(attempts):
        if should_abort is not None:
            try:
                if await should_abort():  # type: ignore[operator]
                    logger.info("tab-capture Allow loop aborted (stop requested)")
                    return
            except Exception:
                pass
        try:
            await asyncio.to_thread(accept_tab_capture_once, display)
        except Exception:
            logger.debug("tab-capture allow attempt %s failed", i + 1, exc_info=True)
        await asyncio.sleep(interval_secs)
