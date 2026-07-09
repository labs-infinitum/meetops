# Operations

## Secrets

Never bake `.env` into images. Required in production:

- `DATABASE_URL`
- `S3_BUCKET` (+ AWS credentials or IAM role)
- `MEETOPS_TOKEN_ENCRYPTION_KEY` (Fernet key)
- `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` for calendar
- `OPENAI_API_KEY` for transcription/notes (optional; AI idle if unset)

## Bot bootstrap

1. Start debug worker with noVNC:  
   `docker compose -f docker-compose.yml -f docker-compose.debug.yml --profile debug up worker-debug`
2. Open `http://localhost:6080`
3. `uv run meetops-cli bot profile-bootstrap --profile default`
4. Sign into the dedicated Meet bot Google account
5. Profiles persist under `./data/chrome-profiles`

## S3 CORS for playback

Allow your app origin to GET objects (or use only presigned URLs which work cross-origin for `<video>` if CORS is set):

```json
[
  {
    "AllowedHeaders": ["*"],
    "AllowedMethods": ["GET", "HEAD"],
    "AllowedOrigins": ["https://app.meetops.pro", "http://localhost:3000"],
    "ExposeHeaders": ["ETag"],
    "MaxAgeSeconds": 3000
  }
]
```

## Graceful shutdown

Worker on SIGTERM/SIGINT: stop scheduling → signal active runs → wait up to 120s for finalize/upload → exit.

## Health

- API: `GET /healthz`
- Postgres: `pg_isready`
- Bot session: `GET /api/v1/bot/session-status` or `meetops-cli bot profile-status`

## Tab capture Allow dialog

When recording starts, Chrome shows **Allow meet.google.com to see this tab?**.

- Debug compose sets `MEETOPS_USE_TAB_CAPTURE_AUTO_ACCEPT=true` so Chromium auto-accepts.
- Otherwise the worker retries xdotool clicks on the Allow button; you can also click **Allow** in noVNC (`:6080`).
- Auto-accept increases bot detectability — leave it off in production unless needed.
