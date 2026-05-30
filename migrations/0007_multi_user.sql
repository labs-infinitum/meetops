-- Multi-user calendar connections and per-user meeting ownership.

CREATE TABLE calendar_connections (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
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

ALTER TABLE meetings ADD COLUMN user_id TEXT REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE meetings ADD COLUMN calendar_connection_id TEXT REFERENCES calendar_connections(id) ON DELETE CASCADE;
ALTER TABLE meetings ADD COLUMN google_event_id TEXT;

ALTER TABLE bot_runs ADD COLUMN user_id TEXT REFERENCES users(id) ON DELETE SET NULL;

-- Migrate singleton google_oauth → calendar_connections (if present).
INSERT INTO calendar_connections (
    id, user_id, provider, email, label, color,
    access_token, refresh_token, expires_at, scope, sync_token,
    created_at, updated_at
)
SELECT
    lower(hex(randomblob(16))),
    (SELECT id FROM users ORDER BY created_at ASC LIMIT 1),
    'google',
    '',
    'Google Calendar',
    '#4285F4',
    access_token,
    refresh_token,
    expires_at,
    scope,
    (SELECT value FROM app_kv WHERE key = 'google_calendar_sync_token'),
    updated_at,
    updated_at
FROM google_oauth
WHERE id = 1;

UPDATE meetings
SET
    calendar_connection_id = (SELECT id FROM calendar_connections ORDER BY created_at ASC LIMIT 1),
    google_event_id = id,
    user_id = (SELECT user_id FROM calendar_connections ORDER BY created_at ASC LIMIT 1)
WHERE calendar_connection_id IS NULL
  AND EXISTS (SELECT 1 FROM calendar_connections);

UPDATE bot_runs
SET user_id = (
    SELECT m.user_id FROM meetings m WHERE m.id = bot_runs.meeting_id
)
WHERE user_id IS NULL;

CREATE INDEX idx_meetings_user_start ON meetings(user_id, start_at);
CREATE INDEX idx_bot_runs_user_status ON bot_runs(user_id, status);
CREATE INDEX idx_meetings_calendar_connection ON meetings(calendar_connection_id);

DROP TABLE google_oauth;
