"""Bot session status (operator)."""

from __future__ import annotations

from datetime import UTC, datetime
from pathlib import Path

from fastapi import APIRouter

from meetops_api.deps import AppSettings, CurrentUser
from meetops_api.schemas import BotSessionStatusOut

router = APIRouter(prefix="/bot", tags=["bot"])


@router.get("/session-status", response_model=BotSessionStatusOut)
async def session_status(user: CurrentUser, settings: AppSettings) -> BotSessionStatusOut:
    profiles = settings.meetops_bot_profiles_dir or (settings.meetops_data_dir / "chrome-profiles")
    default = Path(profiles) / "slot-0"
    state_file = default / "storage-state.json"
    if state_file.exists() or (default / "Default").exists():
        status = "valid"
        detail = f"Profile present at {default}"
    else:
        status = "unknown"
        detail = "No bot profile found. Run: uv run meetops-cli bot profile-bootstrap"
    return BotSessionStatusOut(
        status=status,
        checked_at=datetime.now(UTC),
        detail=detail,
    )