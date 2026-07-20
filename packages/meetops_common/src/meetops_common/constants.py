"""Shared MeetOps constants."""

from __future__ import annotations

BOT_RUN_STATUSES = (
    "queued",
    "joining",
    "in_meeting",
    "recording",
    "uploading",
    "completed",
    "failed",
    "cancelled",
)

TERMINAL_BOT_STATUSES = frozenset({"completed", "failed", "cancelled"})

JOB_KINDS = (
    "transcribe_recording",
    "clean_transcript",
    "generate_meeting_notes",
)

JOB_STATUSES = ("queued", "running", "completed", "failed", "cancelled")

TRANSCRIPT_STATUSES = ("processing", "transcribed", "cleaned", "failed")
SUMMARY_STATUSES = ("processing", "completed", "failed")

TIMELINE_EVENT_TYPES = (
    "participant_joined",
    "participant_left",
    "screen_share_started",
    "screen_share_stopped",
    "active_speaker_changed",
    "participant_snapshot",
    "collector_warning",
)

BOT_SESSION_STATUSES = ("unknown", "valid", "expired", "error")

GOOGLE_CALENDAR_SCOPES = (
    "https://www.googleapis.com/auth/calendar.readonly",
    "https://www.googleapis.com/auth/userinfo.email",
)

DEFAULT_BOT_DISPLAY_NAME = "MeetOps Notetaker"
DEFAULT_EMAIL_FROM_NAME = "MeetOps"

SESSION_TOUCH_THROTTLE_SECS = 60
OAUTH_STATE_TTL_SECS = 600
DUMMY_PASSWORD_HASH = (
    "$argon2id$v=19$m=65536,t=3,p=4$"
    "c29tZXNhbHRzb21lc2FsdA$dGVzdGhhc2h0ZXN0aGFzaHRlc3RoYXNo"
)