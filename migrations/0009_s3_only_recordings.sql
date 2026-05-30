-- Recordings are stored in S3 only.
UPDATE bot_runs SET recording_storage = 's3' WHERE recording_storage = 'local';
