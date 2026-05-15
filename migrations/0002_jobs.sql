CREATE TABLE jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    meeting_id TEXT REFERENCES meetings(id) ON DELETE CASCADE,
    recording_id INTEGER REFERENCES recordings(id) ON DELETE CASCADE,
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
