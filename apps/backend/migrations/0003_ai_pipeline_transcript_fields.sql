ALTER TABLE transcripts
ADD COLUMN IF NOT EXISTS raw_text TEXT,
ADD COLUMN IF NOT EXISTS cleaned_text TEXT,
ADD COLUMN IF NOT EXISTS cleaned_segments_json TEXT;

CREATE INDEX IF NOT EXISTS idx_jobs_bot_run_kind ON jobs(bot_run_id, kind);
