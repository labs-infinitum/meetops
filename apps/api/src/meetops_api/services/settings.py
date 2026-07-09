"""Settings service."""

from __future__ import annotations

from sqlalchemy.ext.asyncio import AsyncSession

from meetops_api.errors import AppError
from meetops_api.schemas import (
    NotificationSettingsPatch,
    ProfilePatch,
    RecordingSettingsPatch,
    SettingsOut,
)
from meetops_common.models import User, UserSettings


class SettingsService:
    def __init__(self, db: AsyncSession) -> None:
        self.db = db

    async def get_or_create(self, user: User) -> UserSettings:
        settings = await self.db.get(UserSettings, user.id)
        if settings is None:
            settings = UserSettings(user_id=user.id)
            self.db.add(settings)
            await self.db.flush()
        return settings

    async def get_out(self, user: User) -> SettingsOut:
        s = await self.get_or_create(user)
        return SettingsOut(
            name=user.name,
            email=user.email,
            username=user.username,
            auto_record_default=s.auto_record_default,
            join_lead_time_secs=s.join_lead_time_secs,
            bot_display_name=s.bot_display_name,
            notify_email_on_notes_ready=s.notify_email_on_notes_ready,
            notify_email_on_recording_failed=s.notify_email_on_recording_failed,
        )

    async def patch_profile(self, user: User, patch: ProfilePatch) -> SettingsOut:
        user.name = patch.name.strip()
        if not user.name:
            raise AppError("invalid_request", "Name is required", 400)
        return await self.get_out(user)

    async def patch_recording(self, user: User, patch: RecordingSettingsPatch) -> SettingsOut:
        s = await self.get_or_create(user)
        if patch.auto_record_default is not None:
            s.auto_record_default = patch.auto_record_default
        if patch.join_lead_time_secs is not None:
            s.join_lead_time_secs = patch.join_lead_time_secs
        if patch.bot_display_name is not None:
            s.bot_display_name = patch.bot_display_name.strip() or None
        await self.db.flush()
        return await self.get_out(user)

    async def patch_notifications(
        self, user: User, patch: NotificationSettingsPatch
    ) -> SettingsOut:
        s = await self.get_or_create(user)
        if patch.notify_email_on_notes_ready is not None:
            s.notify_email_on_notes_ready = patch.notify_email_on_notes_ready
        if patch.notify_email_on_recording_failed is not None:
            s.notify_email_on_recording_failed = patch.notify_email_on_recording_failed
        await self.db.flush()
        return await self.get_out(user)