"""Settings routes."""

from __future__ import annotations

from fastapi import APIRouter

from meetops_api.deps import CurrentUser, DbSession
from meetops_api.schemas import (
    NotificationSettingsPatch,
    ProfilePatch,
    RecordingSettingsPatch,
    SettingsOut,
)
from meetops_api.services.settings import SettingsService

router = APIRouter(prefix="/settings", tags=["settings"])


@router.get("/", response_model=SettingsOut)
async def get_settings(user: CurrentUser, db: DbSession) -> SettingsOut:
    return await SettingsService(db).get_out(user)


@router.patch("/profile", response_model=SettingsOut)
async def patch_profile(
    body: ProfilePatch, user: CurrentUser, db: DbSession
) -> SettingsOut:
    return await SettingsService(db).patch_profile(user, body)


@router.patch("/recording", response_model=SettingsOut)
async def patch_recording(
    body: RecordingSettingsPatch, user: CurrentUser, db: DbSession
) -> SettingsOut:
    return await SettingsService(db).patch_recording(user, body)


@router.patch("/notifications", response_model=SettingsOut)
async def patch_notifications(
    body: NotificationSettingsPatch, user: CurrentUser, db: DbSession
) -> SettingsOut:
    return await SettingsService(db).patch_notifications(user, body)