-- Distinguish local paths from S3 object keys for playback.
ALTER TABLE bot_runs ADD COLUMN recording_storage TEXT NOT NULL DEFAULT 'local';
