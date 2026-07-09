"""Initial MeetOps schema."""

from __future__ import annotations

import sqlalchemy as sa
from alembic import op
from sqlalchemy.dialects import postgresql

revision = "0001_initial"
down_revision = None
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.create_table(
        "users",
        sa.Column("id", sa.Text(), primary_key=True),
        sa.Column("username", sa.Text(), nullable=False),
        sa.Column("email", sa.Text(), nullable=False),
        sa.Column("name", sa.Text(), nullable=False),
        sa.Column("password_hash", sa.Text(), nullable=False),
        sa.Column("email_verified_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
    )
    op.create_index("idx_users_username_lower", "users", [sa.text("lower(username)")], unique=True)
    op.create_index("idx_users_email_lower", "users", [sa.text("lower(email)")], unique=True)

    op.create_table(
        "sessions",
        sa.Column("token", sa.Text(), primary_key=True),
        sa.Column("user_id", sa.Text(), sa.ForeignKey("users.id", ondelete="CASCADE"), nullable=False),
        sa.Column("expires_at", sa.DateTime(timezone=True), nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("last_used_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("user_agent", sa.Text(), nullable=True),
        sa.Column("ip", sa.Text(), nullable=True),
    )
    op.create_index("idx_sessions_user_id", "sessions", ["user_id"])
    op.create_index("idx_sessions_expires_at", "sessions", ["expires_at"])

    op.create_table(
        "auth_email_tokens",
        sa.Column("token", sa.Text(), primary_key=True),
        sa.Column("user_id", sa.Text(), sa.ForeignKey("users.id", ondelete="CASCADE"), nullable=False),
        sa.Column("purpose", sa.Text(), nullable=False),
        sa.Column("expires_at", sa.DateTime(timezone=True), nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
    )
    op.create_index("idx_auth_email_tokens_user_purpose", "auth_email_tokens", ["user_id", "purpose"])
    op.create_index("idx_auth_email_tokens_expires", "auth_email_tokens", ["expires_at"])

    op.create_table(
        "calendar_connections",
        sa.Column("id", sa.Text(), primary_key=True),
        sa.Column("user_id", sa.Text(), sa.ForeignKey("users.id", ondelete="CASCADE"), nullable=False),
        sa.Column("provider", sa.Text(), nullable=False),
        sa.Column("email", sa.Text(), nullable=False),
        sa.Column("label", sa.Text(), nullable=True),
        sa.Column("color", sa.Text(), nullable=True),
        sa.Column("access_token", sa.Text(), nullable=False),
        sa.Column("refresh_token", sa.Text(), nullable=False),
        sa.Column("expires_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("scope", sa.Text(), nullable=True),
        sa.Column("sync_token", sa.Text(), nullable=True),
        sa.Column("last_synced_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("bot_session_status", sa.Text(), nullable=False, server_default="unknown"),
        sa.Column("bot_session_checked_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.UniqueConstraint("provider", "email", name="uq_calendar_provider_email"),
    )
    op.create_index("idx_calendar_connections_user_id", "calendar_connections", ["user_id"])

    op.create_table(
        "user_settings",
        sa.Column("user_id", sa.Text(), sa.ForeignKey("users.id", ondelete="CASCADE"), primary_key=True),
        sa.Column("auto_record_default", sa.Boolean(), nullable=False, server_default=sa.text("true")),
        sa.Column("join_lead_time_secs", sa.Integer(), nullable=False, server_default="60"),
        sa.Column("bot_display_name", sa.Text(), nullable=True),
        sa.Column("notify_email_on_notes_ready", sa.Boolean(), nullable=False, server_default=sa.text("true")),
        sa.Column(
            "notify_email_on_recording_failed",
            sa.Boolean(),
            nullable=False,
            server_default=sa.text("true"),
        ),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
    )

    op.create_table(
        "meetings",
        sa.Column("id", sa.Text(), primary_key=True),
        sa.Column("user_id", sa.Text(), sa.ForeignKey("users.id", ondelete="CASCADE"), nullable=False),
        sa.Column(
            "calendar_connection_id",
            sa.Text(),
            sa.ForeignKey("calendar_connections.id", ondelete="CASCADE"),
            nullable=False,
        ),
        sa.Column("google_event_id", sa.Text(), nullable=False),
        sa.Column("title", sa.Text(), nullable=False),
        sa.Column("description", sa.Text(), nullable=True),
        sa.Column("meet_url", sa.Text(), nullable=True),
        sa.Column("start_at", sa.DateTime(timezone=True), nullable=False),
        sa.Column("end_at", sa.DateTime(timezone=True), nullable=False),
        sa.Column("auto_join", sa.Boolean(), nullable=False, server_default=sa.text("true")),
        sa.Column("attendees_json", postgresql.JSONB(), nullable=True),
        sa.Column("calendar_etag", sa.Text(), nullable=True),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
    )
    op.create_index("idx_meetings_user_start", "meetings", ["user_id", "start_at"])
    op.create_index("idx_meetings_start_at", "meetings", ["start_at"])
    op.create_index("idx_meetings_calendar_connection", "meetings", ["calendar_connection_id"])

    op.create_table(
        "bot_runs",
        sa.Column("id", sa.BigInteger(), primary_key=True, autoincrement=True),
        sa.Column("meeting_id", sa.Text(), sa.ForeignKey("meetings.id", ondelete="SET NULL"), nullable=True),
        sa.Column("meeting_start_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("user_id", sa.Text(), sa.ForeignKey("users.id", ondelete="CASCADE"), nullable=False),
        sa.Column("status", sa.Text(), nullable=False),
        sa.Column("error", sa.Text(), nullable=True),
        sa.Column("trigger", sa.Text(), nullable=False, server_default="scheduled"),
        sa.Column("chrome_profile_path", sa.Text(), nullable=True),
        sa.Column("display", sa.Text(), nullable=True),
        sa.Column("recording_path", sa.Text(), nullable=True),
        sa.Column("audio_path", sa.Text(), nullable=True),
        sa.Column("meet_url", sa.Text(), nullable=True),
        sa.Column("started_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("ended_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
    )
    op.create_index(
        "uq_bot_runs_meeting_start",
        "bot_runs",
        ["meeting_id", "meeting_start_at"],
        unique=True,
        postgresql_where=sa.text("meeting_id IS NOT NULL"),
    )
    op.create_index("idx_bot_runs_status", "bot_runs", ["status"])
    op.create_index("idx_bot_runs_user_created", "bot_runs", ["user_id", sa.text("created_at DESC")])

    op.create_table(
        "meeting_timeline_events",
        sa.Column("id", sa.BigInteger(), primary_key=True, autoincrement=True),
        sa.Column("bot_run_id", sa.BigInteger(), sa.ForeignKey("bot_runs.id", ondelete="CASCADE"), nullable=False),
        sa.Column("event_type", sa.Text(), nullable=False),
        sa.Column("occurred_at", sa.DateTime(timezone=True), nullable=False),
        sa.Column("relative_ms", sa.Integer(), nullable=True),
        sa.Column("participant_id", sa.Text(), nullable=True),
        sa.Column("participant_name", sa.Text(), nullable=True),
        sa.Column("payload_json", postgresql.JSONB(), nullable=True),
    )
    op.create_index(
        "idx_timeline_bot_run_occurred",
        "meeting_timeline_events",
        ["bot_run_id", "occurred_at"],
    )
    op.create_index(
        "idx_timeline_bot_run_type",
        "meeting_timeline_events",
        ["bot_run_id", "event_type"],
    )

    op.create_table(
        "transcripts",
        sa.Column("id", sa.BigInteger(), primary_key=True, autoincrement=True),
        sa.Column(
            "bot_run_id",
            sa.BigInteger(),
            sa.ForeignKey("bot_runs.id", ondelete="CASCADE"),
            nullable=False,
            unique=True,
        ),
        sa.Column("status", sa.Text(), nullable=False),
        sa.Column("raw_text", sa.Text(), nullable=True),
        sa.Column("cleaned_text", sa.Text(), nullable=True),
        sa.Column("segments_json", postgresql.JSONB(), nullable=True),
        sa.Column("cleaned_segments_json", postgresql.JSONB(), nullable=True),
        sa.Column("language", sa.Text(), nullable=True),
        sa.Column("error", sa.Text(), nullable=True),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
    )

    op.create_table(
        "summaries",
        sa.Column("id", sa.BigInteger(), primary_key=True, autoincrement=True),
        sa.Column(
            "transcript_id",
            sa.BigInteger(),
            sa.ForeignKey("transcripts.id", ondelete="CASCADE"),
            nullable=False,
            unique=True,
        ),
        sa.Column("status", sa.Text(), nullable=False),
        sa.Column("tldr", sa.Text(), nullable=True),
        sa.Column("summary_md", sa.Text(), nullable=True),
        sa.Column("error", sa.Text(), nullable=True),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
    )

    op.create_table(
        "action_items",
        sa.Column("id", sa.BigInteger(), primary_key=True, autoincrement=True),
        sa.Column("summary_id", sa.BigInteger(), sa.ForeignKey("summaries.id", ondelete="CASCADE"), nullable=False),
        sa.Column("bot_run_id", sa.BigInteger(), sa.ForeignKey("bot_runs.id", ondelete="CASCADE"), nullable=False),
        sa.Column("user_id", sa.Text(), sa.ForeignKey("users.id", ondelete="CASCADE"), nullable=False),
        sa.Column("text", sa.Text(), nullable=False),
        sa.Column("owner", sa.Text(), nullable=True),
        sa.Column("due", sa.Text(), nullable=True),
        sa.Column("completed", sa.Boolean(), nullable=False, server_default=sa.text("false")),
        sa.Column("completed_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
    )
    op.create_index(
        "idx_action_items_user_completed",
        "action_items",
        ["user_id", "completed", sa.text("created_at DESC")],
    )

    op.create_table(
        "jobs",
        sa.Column("id", sa.BigInteger(), primary_key=True, autoincrement=True),
        sa.Column("kind", sa.Text(), nullable=False),
        sa.Column("status", sa.Text(), nullable=False),
        sa.Column("meeting_id", sa.Text(), sa.ForeignKey("meetings.id", ondelete="SET NULL"), nullable=True),
        sa.Column("bot_run_id", sa.BigInteger(), sa.ForeignKey("bot_runs.id", ondelete="CASCADE"), nullable=True),
        sa.Column(
            "transcript_id",
            sa.BigInteger(),
            sa.ForeignKey("transcripts.id", ondelete="CASCADE"),
            nullable=True,
        ),
        sa.Column("payload_json", postgresql.JSONB(), nullable=True),
        sa.Column("attempts", sa.Integer(), nullable=False, server_default="0"),
        sa.Column("run_after", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("locked_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("locked_by", sa.Text(), nullable=True),
        sa.Column("last_error", sa.Text(), nullable=True),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
    )
    op.create_index("idx_jobs_status_run_after", "jobs", ["status", "run_after"])
    op.create_index("idx_jobs_kind_status", "jobs", ["kind", "status"])
    op.create_index("idx_jobs_bot_run_kind", "jobs", ["bot_run_id", "kind"])

    op.create_table(
        "app_kv",
        sa.Column("key", sa.Text(), primary_key=True),
        sa.Column("value", sa.Text(), nullable=False),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
    )


def downgrade() -> None:
    for table in (
        "app_kv",
        "jobs",
        "action_items",
        "summaries",
        "transcripts",
        "meeting_timeline_events",
        "bot_runs",
        "meetings",
        "user_settings",
        "calendar_connections",
        "auth_email_tokens",
        "sessions",
        "users",
    ):
        op.drop_table(table)
