"""SQLAlchemy 2.0 ORM models for MeetOps."""

from __future__ import annotations

from datetime import UTC, datetime
from typing import Any

from sqlalchemy import (
    BigInteger,
    Boolean,
    DateTime,
    ForeignKey,
    Index,
    Integer,
    Text,
    UniqueConstraint,
    func,
    text,
)
from sqlalchemy.dialects.postgresql import JSONB
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, relationship


class Base(DeclarativeBase):
    pass


def _utcnow() -> datetime:

    return datetime.now(UTC)


class User(Base):
    __tablename__ = "users"

    id: Mapped[str] = mapped_column(Text, primary_key=True)
    username: Mapped[str] = mapped_column(Text, nullable=False)
    email: Mapped[str] = mapped_column(Text, nullable=False)
    name: Mapped[str] = mapped_column(Text, nullable=False)
    password_hash: Mapped[str] = mapped_column(Text, nullable=False)
    email_verified_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True))
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now(), onupdate=func.now()
    )

    settings: Mapped[UserSettings | None] = relationship(back_populates="user", uselist=False)
    sessions: Mapped[list[Session]] = relationship(back_populates="user")


class Session(Base):
    __tablename__ = "sessions"

    token: Mapped[str] = mapped_column(Text, primary_key=True)
    user_id: Mapped[str] = mapped_column(Text, ForeignKey("users.id", ondelete="CASCADE"))
    expires_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )
    last_used_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )
    user_agent: Mapped[str | None] = mapped_column(Text)
    ip: Mapped[str | None] = mapped_column(Text)

    user: Mapped[User] = relationship(back_populates="sessions")


class AuthEmailToken(Base):
    __tablename__ = "auth_email_tokens"

    token: Mapped[str] = mapped_column(Text, primary_key=True)
    user_id: Mapped[str] = mapped_column(Text, ForeignKey("users.id", ondelete="CASCADE"))
    purpose: Mapped[str] = mapped_column(Text, nullable=False)  # verify_email | reset_password
    expires_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )


class CalendarConnection(Base):
    __tablename__ = "calendar_connections"
    __table_args__ = (UniqueConstraint("provider", "email", name="uq_calendar_provider_email"),)

    id: Mapped[str] = mapped_column(Text, primary_key=True)
    user_id: Mapped[str] = mapped_column(Text, ForeignKey("users.id", ondelete="CASCADE"))
    provider: Mapped[str] = mapped_column(Text, nullable=False, default="google")
    email: Mapped[str] = mapped_column(Text, nullable=False)
    label: Mapped[str | None] = mapped_column(Text)
    color: Mapped[str | None] = mapped_column(Text, default="#4285F4")
    access_token: Mapped[str] = mapped_column(Text, nullable=False)
    refresh_token: Mapped[str] = mapped_column(Text, nullable=False)
    expires_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True))
    scope: Mapped[str | None] = mapped_column(Text)
    sync_token: Mapped[str | None] = mapped_column(Text)
    last_synced_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True))
    bot_session_status: Mapped[str] = mapped_column(Text, nullable=False, default="unknown")
    bot_session_checked_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True))
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now(), onupdate=func.now()
    )


class UserSettings(Base):
    __tablename__ = "user_settings"

    user_id: Mapped[str] = mapped_column(
        Text, ForeignKey("users.id", ondelete="CASCADE"), primary_key=True
    )
    auto_record_default: Mapped[bool] = mapped_column(Boolean, nullable=False, default=True)
    join_lead_time_secs: Mapped[int] = mapped_column(Integer, nullable=False, default=60)
    bot_display_name: Mapped[str | None] = mapped_column(Text)
    notify_email_on_notes_ready: Mapped[bool] = mapped_column(Boolean, nullable=False, default=True)
    notify_email_on_recording_failed: Mapped[bool] = mapped_column(
        Boolean, nullable=False, default=True
    )
    languages_json: Mapped[Any] = mapped_column(
        JSONB, nullable=False, server_default=text("'[\"en\"]'::jsonb")
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now(), onupdate=func.now()
    )

    user: Mapped[User] = relationship(back_populates="settings")


class Meeting(Base):
    __tablename__ = "meetings"

    id: Mapped[str] = mapped_column(Text, primary_key=True)
    user_id: Mapped[str] = mapped_column(Text, ForeignKey("users.id", ondelete="CASCADE"))
    calendar_connection_id: Mapped[str] = mapped_column(
        Text, ForeignKey("calendar_connections.id", ondelete="CASCADE")
    )
    google_event_id: Mapped[str] = mapped_column(Text, nullable=False)
    title: Mapped[str] = mapped_column(Text, nullable=False)
    description: Mapped[str | None] = mapped_column(Text)
    meet_url: Mapped[str | None] = mapped_column(Text)
    start_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)
    end_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)
    auto_join: Mapped[bool] = mapped_column(Boolean, nullable=False, default=True)
    attendees_json: Mapped[Any | None] = mapped_column(JSONB)
    calendar_etag: Mapped[str | None] = mapped_column(Text)
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now(), onupdate=func.now()
    )


class BotRun(Base):
    __tablename__ = "bot_runs"
    __table_args__ = (
        Index(
            "uq_bot_runs_meeting_start",
            "meeting_id",
            "meeting_start_at",
            unique=True,
            postgresql_where=text("meeting_id IS NOT NULL"),
        ),
    )

    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    meeting_id: Mapped[str | None] = mapped_column(
        Text, ForeignKey("meetings.id", ondelete="SET NULL")
    )
    meeting_start_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True))
    user_id: Mapped[str] = mapped_column(Text, ForeignKey("users.id", ondelete="CASCADE"))
    status: Mapped[str] = mapped_column(Text, nullable=False, default="queued")
    error: Mapped[str | None] = mapped_column(Text)
    trigger: Mapped[str] = mapped_column(Text, nullable=False, default="scheduled")
    chrome_profile_path: Mapped[str | None] = mapped_column(Text)
    display: Mapped[str | None] = mapped_column(Text)
    recording_path: Mapped[str | None] = mapped_column(Text)
    audio_path: Mapped[str | None] = mapped_column(Text)
    meet_url: Mapped[str | None] = mapped_column(Text)
    started_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True))
    ended_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True))
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now(), onupdate=func.now()
    )


class MeetingTimelineEvent(Base):
    __tablename__ = "meeting_timeline_events"

    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    bot_run_id: Mapped[int] = mapped_column(
        BigInteger, ForeignKey("bot_runs.id", ondelete="CASCADE")
    )
    event_type: Mapped[str] = mapped_column(Text, nullable=False)
    occurred_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)
    relative_ms: Mapped[int | None] = mapped_column(Integer)
    participant_id: Mapped[str | None] = mapped_column(Text)
    participant_name: Mapped[str | None] = mapped_column(Text)
    payload_json: Mapped[Any | None] = mapped_column(JSONB)


class Transcript(Base):
    __tablename__ = "transcripts"

    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    bot_run_id: Mapped[int] = mapped_column(
        BigInteger, ForeignKey("bot_runs.id", ondelete="CASCADE"), unique=True
    )
    status: Mapped[str] = mapped_column(Text, nullable=False, default="processing")
    raw_text: Mapped[str | None] = mapped_column(Text)
    cleaned_text: Mapped[str | None] = mapped_column(Text)
    segments_json: Mapped[Any | None] = mapped_column(JSONB)
    cleaned_segments_json: Mapped[Any | None] = mapped_column(JSONB)
    language: Mapped[str | None] = mapped_column(Text)
    error: Mapped[str | None] = mapped_column(Text)
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now(), onupdate=func.now()
    )


class Summary(Base):
    __tablename__ = "summaries"

    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    transcript_id: Mapped[int] = mapped_column(
        BigInteger, ForeignKey("transcripts.id", ondelete="CASCADE"), unique=True
    )
    status: Mapped[str] = mapped_column(Text, nullable=False, default="processing")
    tldr: Mapped[str | None] = mapped_column(Text)
    summary_md: Mapped[str | None] = mapped_column(Text)
    error: Mapped[str | None] = mapped_column(Text)
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now(), onupdate=func.now()
    )


class ActionItem(Base):
    __tablename__ = "action_items"

    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    summary_id: Mapped[int] = mapped_column(
        BigInteger, ForeignKey("summaries.id", ondelete="CASCADE")
    )
    bot_run_id: Mapped[int] = mapped_column(
        BigInteger, ForeignKey("bot_runs.id", ondelete="CASCADE")
    )
    user_id: Mapped[str] = mapped_column(Text, ForeignKey("users.id", ondelete="CASCADE"))
    text: Mapped[str] = mapped_column(Text, nullable=False)
    owner: Mapped[str | None] = mapped_column(Text)
    due: Mapped[str | None] = mapped_column(Text)
    completed: Mapped[bool] = mapped_column(Boolean, nullable=False, default=False)
    completed_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True))
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now(), onupdate=func.now()
    )


class Job(Base):
    __tablename__ = "jobs"

    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    kind: Mapped[str] = mapped_column(Text, nullable=False)
    status: Mapped[str] = mapped_column(Text, nullable=False, default="queued")
    meeting_id: Mapped[str | None] = mapped_column(Text, ForeignKey("meetings.id", ondelete="SET NULL"))
    bot_run_id: Mapped[int | None] = mapped_column(
        BigInteger, ForeignKey("bot_runs.id", ondelete="CASCADE")
    )
    transcript_id: Mapped[int | None] = mapped_column(
        BigInteger, ForeignKey("transcripts.id", ondelete="CASCADE")
    )
    payload_json: Mapped[Any | None] = mapped_column(JSONB)
    attempts: Mapped[int] = mapped_column(Integer, nullable=False, default=0)
    run_after: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )
    locked_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True))
    locked_by: Mapped[str | None] = mapped_column(Text)
    last_error: Mapped[str | None] = mapped_column(Text)
    created_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now(), onupdate=func.now()
    )


class AppKv(Base):
    __tablename__ = "app_kv"

    key: Mapped[str] = mapped_column(Text, primary_key=True)
    value: Mapped[str] = mapped_column(Text, nullable=False)
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now(), onupdate=func.now()
    )