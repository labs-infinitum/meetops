CREATE TABLE meeting_timeline_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bot_run_id INTEGER NOT NULL REFERENCES bot_runs(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    occurred_at INTEGER NOT NULL,
    relative_ms INTEGER NOT NULL,
    participant_id TEXT,
    participant_name TEXT,
    payload_json TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_meeting_timeline_events_run_time ON meeting_timeline_events(bot_run_id, occurred_at);
CREATE INDEX idx_meeting_timeline_events_run_type ON meeting_timeline_events(bot_run_id, event_type);
