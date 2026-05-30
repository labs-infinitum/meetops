-- Greenfield schema (delete data/meetings.db when upgrading from older migration chains).

CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    email TEXT NOT NULL UNIQUE COLLATE NOCASE,
    name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_users_email ON users(email);

CREATE TABLE sessions (
    token TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);

CREATE TABLE calendar_connections (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    email TEXT NOT NULL COLLATE NOCASE,
    label TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT '#4285F4',
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    scope TEXT NOT NULL,
    sync_token TEXT,
    last_synced_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(provider, email)
);

CREATE INDEX idx_calendar_connections_user_id ON calendar_connections(user_id);

CREATE TABLE user_settings (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    auto_record_default INTEGER NOT NULL DEFAULT 1,
    join_lead_time_secs INTEGER,
    bot_display_name TEXT,
    notify_summary_ready INTEGER NOT NULL DEFAULT 1,
    notify_recording_failed INTEGER NOT NULL DEFAULT 1,
    notify_daily_digest INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE meetings (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    organizer_email TEXT,
    meet_url TEXT NOT NULL,
    start_at INTEGER NOT NULL,
    end_at INTEGER NOT NULL,
    attendees_json TEXT,
    calendar_etag TEXT,
    auto_join INTEGER NOT NULL DEFAULT 1,
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    calendar_connection_id TEXT REFERENCES calendar_connections(id) ON DELETE CASCADE,
    google_event_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_meetings_start_at ON meetings(start_at);
CREATE INDEX idx_meetings_user_start ON meetings(user_id, start_at);
CREATE INDEX idx_meetings_calendar_connection ON meetings(calendar_connection_id);

CREATE TABLE bot_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    meeting_start_at INTEGER NOT NULL,
    status TEXT NOT NULL,
    error TEXT,
    trigger_after INTEGER NOT NULL,
    started_at INTEGER,
    ended_at INTEGER,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recording_path TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(meeting_id, meeting_start_at)
);

CREATE INDEX idx_bot_runs_status ON bot_runs(status);
CREATE INDEX idx_bot_runs_created_at ON bot_runs(created_at DESC);
CREATE INDEX idx_bot_runs_user_status ON bot_runs(user_id, status);

CREATE TABLE app_kv (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE transcripts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bot_run_id INTEGER NOT NULL UNIQUE REFERENCES bot_runs(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    text TEXT,
    segments_json TEXT,
    language TEXT,
    error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transcript_id INTEGER NOT NULL UNIQUE REFERENCES transcripts(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    tldr TEXT,
    summary_md TEXT,
    action_items_json TEXT,
    error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    meeting_id TEXT REFERENCES meetings(id) ON DELETE CASCADE,
    bot_run_id INTEGER REFERENCES bot_runs(id) ON DELETE CASCADE,
    transcript_id INTEGER REFERENCES transcripts(id) ON DELETE CASCADE,
    payload_json TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    run_after INTEGER NOT NULL,
    locked_at INTEGER,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_jobs_status_run_after ON jobs(status, run_after);
CREATE INDEX idx_jobs_kind_status ON jobs(kind, status);
