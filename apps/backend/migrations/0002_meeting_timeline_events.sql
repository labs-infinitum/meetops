CREATE TABLE meeting_timeline_events (
    id BIGSERIAL PRIMARY KEY,
    bot_run_id BIGINT NOT NULL REFERENCES bot_runs(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    occurred_at BIGINT NOT NULL,
    relative_ms BIGINT NOT NULL,
    participant_id TEXT,
    participant_name TEXT,
    payload_json TEXT,
    created_at BIGINT NOT NULL
);

CREATE INDEX idx_meeting_timeline_events_run_time ON meeting_timeline_events(bot_run_id, occurred_at);
CREATE INDEX idx_meeting_timeline_events_run_type ON meeting_timeline_events(bot_run_id, event_type);
