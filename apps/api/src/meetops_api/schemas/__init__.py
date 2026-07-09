"""Shared Pydantic schemas."""

from __future__ import annotations

from datetime import datetime
from typing import Any

from pydantic import BaseModel, ConfigDict, EmailStr, Field


class ORMModel(BaseModel):
    model_config = ConfigDict(from_attributes=True)


class UserOut(ORMModel):
    id: str
    username: str
    email: EmailStr
    name: str
    email_verified_at: datetime | None = None
    created_at: datetime


class SignupRequest(BaseModel):
    name: str = Field(min_length=1, max_length=200)
    email: EmailStr
    username: str = Field(min_length=3, max_length=64)
    password: str = Field(min_length=8, max_length=200)


class SignupResponse(BaseModel):
    user: UserOut
    email_verification_required: bool


class LoginRequest(BaseModel):
    username: str
    password: str


class LoginResponse(BaseModel):
    user: UserOut


class MeResponse(BaseModel):
    user: UserOut


class PasswordResetRequest(BaseModel):
    email: EmailStr


class PasswordResetConfirm(BaseModel):
    token: str
    new_password: str = Field(min_length=8, max_length=200)


class EmailVerifyRequest(BaseModel):
    token: str


class EmailResendRequest(BaseModel):
    email: EmailStr


class MessageResponse(BaseModel):
    message: str


class SettingsOut(BaseModel):
    name: str
    email: EmailStr
    username: str
    auto_record_default: bool
    join_lead_time_secs: int
    bot_display_name: str | None
    notify_email_on_notes_ready: bool
    notify_email_on_recording_failed: bool


class ProfilePatch(BaseModel):
    name: str = Field(min_length=1, max_length=200)


class RecordingSettingsPatch(BaseModel):
    auto_record_default: bool | None = None
    join_lead_time_secs: int | None = Field(default=None, ge=0, le=3600)
    bot_display_name: str | None = Field(default=None, max_length=100)


class NotificationSettingsPatch(BaseModel):
    notify_email_on_notes_ready: bool | None = None
    notify_email_on_recording_failed: bool | None = None


class CalendarConnectionOut(ORMModel):
    id: str
    provider: str
    email: str
    label: str | None
    color: str | None
    last_synced_at: datetime | None
    bot_session_status: str
    created_at: datetime


class OAuthStartRequest(BaseModel):
    provider: str = "google"
    label: str | None = None
    color: str | None = None


class OAuthStartResponse(BaseModel):
    redirect_url: str


class SyncResult(BaseModel):
    seen: int
    upserted: int
    deleted: int
    last_synced_at: datetime | None


class MeetingOut(ORMModel):
    id: str
    title: str
    description: str | None
    meet_url: str | None
    start_at: datetime
    end_at: datetime
    auto_join: bool
    calendar_connection_id: str
    attendees_json: Any | None = None


class MeetingPatch(BaseModel):
    auto_record: bool


class DashboardOut(BaseModel):
    upcoming_count: int
    recordings_count: int
    notes_count: int
    open_todos_count: int
    upcoming: list[MeetingOut]
    latest_summary: dict[str, Any] | None = None


class BotRunOut(ORMModel):
    id: str
    meeting_id: str | None
    status: str
    error: str | None
    trigger: str
    recording_path: str | None
    meet_url: str | None = None
    meeting_title: str | None = None
    meeting_start_at: datetime | None = None
    duration_secs: int | None = None
    started_at: datetime | None
    ended_at: datetime | None
    created_at: datetime

    @classmethod
    def from_orm_run(
        cls,
        run: Any,
        *,
        meeting_title: str | None = None,
    ) -> BotRunOut:
        duration_secs: int | None = None
        if run.started_at and run.ended_at:
            duration_secs = max(
                0, int((run.ended_at - run.started_at).total_seconds())
            )
        return cls(
            id=str(run.id),
            meeting_id=run.meeting_id,
            status=run.status,
            error=run.error,
            trigger=run.trigger,
            recording_path=run.recording_path,
            meet_url=getattr(run, "meet_url", None),
            meeting_title=meeting_title,
            meeting_start_at=getattr(run, "meeting_start_at", None),
            duration_secs=duration_secs,
            started_at=run.started_at,
            ended_at=run.ended_at,
            created_at=run.created_at,
        )


class AdHocRequest(BaseModel):
    meet_url: str = Field(min_length=8, max_length=2000)


class PlaybackUrlOut(BaseModel):
    url: str
    expires_at: datetime


class TimelineEventOut(ORMModel):
    id: str
    event_type: str
    occurred_at: datetime
    relative_ms: int | None
    participant_id: str | None
    participant_name: str | None
    payload_json: Any | None = None

    @classmethod
    def from_orm(cls, ev: Any) -> TimelineEventOut:
        return cls(
            id=str(ev.id),
            event_type=ev.event_type,
            occurred_at=ev.occurred_at,
            relative_ms=ev.relative_ms,
            participant_id=ev.participant_id,
            participant_name=ev.participant_name,
            payload_json=ev.payload_json,
        )


class NoteOut(BaseModel):
    id: str
    transcript_id: str
    bot_run_id: str
    status: str
    transcript_status: str | None = None
    summary_status: str | None = None
    tldr: str | None
    summary_md: str | None
    raw_transcript: str | None = None
    cleaned_transcript: str | None = None
    meeting_title: str | None = None
    error: str | None = None
    created_at: datetime
    action_items: list[TodoOut] = []


class TodoOut(BaseModel):
    id: str
    text: str
    owner: str | None
    due: str | None
    completed: bool
    completed_at: datetime | None
    bot_run_id: str
    created_at: datetime

    @classmethod
    def from_orm(cls, item: Any) -> TodoOut:
        return cls(
            id=str(item.id),
            text=item.text,
            owner=item.owner,
            due=item.due,
            completed=item.completed,
            completed_at=item.completed_at,
            bot_run_id=str(item.bot_run_id),
            created_at=item.created_at,
        )


class TodoPatch(BaseModel):
    completed: bool


class BotSessionStatusOut(BaseModel):
    status: str
    checked_at: datetime | None
    detail: str | None = None


# Resolve forward refs for NoteOut
NoteOut.model_rebuild()