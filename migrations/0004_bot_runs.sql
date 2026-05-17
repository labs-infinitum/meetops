CREATE TABLE bot_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    meeting_start_at INTEGER NOT NULL,
    status TEXT NOT NULL,
    error TEXT,
    trigger_after INTEGER NOT NULL,
    started_at INTEGER,
    ended_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(meeting_id, meeting_start_at)
);

CREATE INDEX idx_bot_runs_status ON bot_runs(status);
CREATE INDEX idx_bot_runs_created_at ON bot_runs(created_at DESC);
