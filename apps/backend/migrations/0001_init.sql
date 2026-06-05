-- PostgreSQL schema for AI Meeting Notetaker (production RDS).

CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL,
    email TEXT NOT NULL,
    name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE UNIQUE INDEX idx_users_username_lower ON users (lower(username));
CREATE UNIQUE INDEX idx_users_email_lower ON users (lower(email));
CREATE INDEX idx_users_email ON users(email);

CREATE TABLE sessions (
    token TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    last_used_at BIGINT NOT NULL
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);

CREATE TABLE calendar_connections (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    email TEXT NOT NULL,
    label TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT '#4285F4',
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    scope TEXT NOT NULL,
    sync_token TEXT,
    last_synced_at BIGINT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE(provider, email)
);

CREATE INDEX idx_calendar_connections_user_id ON calendar_connections(user_id);

CREATE TABLE user_settings (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    auto_record_default BOOLEAN NOT NULL DEFAULT TRUE,
    join_lead_time_secs BIGINT,
    bot_display_name TEXT,
    notify_summary_ready BOOLEAN NOT NULL DEFAULT TRUE,
    notify_recording_failed BOOLEAN NOT NULL DEFAULT TRUE,
    notify_daily_digest BOOLEAN NOT NULL DEFAULT FALSE,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE meetings (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    organizer_email TEXT,
    meet_url TEXT NOT NULL,
    start_at BIGINT NOT NULL,
    end_at BIGINT NOT NULL,
    attendees_json TEXT,
    calendar_etag TEXT,
    auto_join BOOLEAN NOT NULL DEFAULT TRUE,
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    calendar_connection_id TEXT REFERENCES calendar_connections(id) ON DELETE CASCADE,
    google_event_id TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX idx_meetings_start_at ON meetings(start_at);
CREATE INDEX idx_meetings_user_start ON meetings(user_id, start_at);
CREATE INDEX idx_meetings_calendar_connection ON meetings(calendar_connection_id);

CREATE TABLE bot_runs (
    id BIGSERIAL PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    meeting_start_at BIGINT NOT NULL,
    status TEXT NOT NULL,
    error TEXT,
    trigger_after BIGINT NOT NULL,
    started_at BIGINT,
    ended_at BIGINT,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recording_path TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE(meeting_id, meeting_start_at)
);

CREATE INDEX idx_bot_runs_status ON bot_runs(status);
CREATE INDEX idx_bot_runs_created_at ON bot_runs(created_at DESC);
CREATE INDEX idx_bot_runs_user_status ON bot_runs(user_id, status);

CREATE TABLE app_kv (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE transcripts (
    id BIGSERIAL PRIMARY KEY,
    bot_run_id BIGINT NOT NULL UNIQUE REFERENCES bot_runs(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    text TEXT,
    segments_json TEXT,
    language TEXT,
    error TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE summaries (
    id BIGSERIAL PRIMARY KEY,
    transcript_id BIGINT NOT NULL UNIQUE REFERENCES transcripts(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    tldr TEXT,
    summary_md TEXT,
    action_items_json TEXT,
    error TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE jobs (
    id BIGSERIAL PRIMARY KEY,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    meeting_id TEXT REFERENCES meetings(id) ON DELETE CASCADE,
    bot_run_id BIGINT REFERENCES bot_runs(id) ON DELETE CASCADE,
    transcript_id BIGINT REFERENCES transcripts(id) ON DELETE CASCADE,
    payload_json TEXT,
    attempts BIGINT NOT NULL DEFAULT 0,
    run_after BIGINT NOT NULL,
    locked_at BIGINT,
    last_error TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX idx_jobs_status_run_after ON jobs(status, run_after);
CREATE INDEX idx_jobs_kind_status ON jobs(kind, status);
