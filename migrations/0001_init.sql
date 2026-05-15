CREATE TABLE google_oauth (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    scope TEXT NOT NULL,
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
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_meetings_start_at ON meetings(start_at);

CREATE TABLE recordings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    bot_started_at INTEGER,
    bot_ended_at INTEGER,
    video_path TEXT,
    audio_path TEXT,
    duration_secs INTEGER,
    error TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_recordings_status ON recordings(status);

CREATE TABLE transcripts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recording_id INTEGER NOT NULL UNIQUE REFERENCES recordings(id) ON DELETE CASCADE,
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
