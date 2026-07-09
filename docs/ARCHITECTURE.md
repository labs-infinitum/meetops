# Architecture

## Process model

- **`api`** — FastAPI HTTP/JSON REST, OAuth callback, cookie sessions.
- **`worker`** — Scheduler, bot recordings, calendar sync, AI jobs, cleanup.
- Shared code lives in `meetops_api` + `meetops_common` (worker depends on api package).

Dev can run both processes locally; Compose always splits them.

## Bot identity

**MVP:** one dedicated Google Workspace bot account. Chrome profiles are **per-slot** (`slot-0`, `slot-1`, …) under `MEETOPS_BOT_PROFILES_DIR`, seeded from `default/` after bootstrap.

Calendar OAuth tokens are per-user and **not** used to join Meet.

## CSRF

Session cookie: `HttpOnly`, `Secure` (prod), `SameSite=Lax`. Production SPA is same-site via Caddy reverse proxy (`/api`, `/auth` → api). Dev: Vite proxies those paths to `:8000`.

## IDs

- UUID strings: users, calendar connections, meetings.
- Bigint PKs serialized as **strings** in JSON: bot runs, jobs, timeline, transcripts, summaries, action items.

## Tab-capture flags

Default: **no** `--use-fake-ui-for-media-stream` / auto-accept flags. Prefer CDP `grantPermissions` + xdotool for Allow dialogs.

Set `MEETOPS_USE_TAB_CAPTURE_AUTO_ACCEPT=true` only if join success requires it; this increases detectability.

## Auth

Opaque session tokens in DB; cookie stores only the token. Argon2id passwords. Generic signup errors. Rate limits on auth endpoints. Token encryption required in production (`MEETOPS_TOKEN_ENCRYPTION_KEY`).

## Recording concurrency

`SlotAllocator` with `MEETOPS_BOT_CONCURRENCY` (default 2): each slot has its own `DISPLAY` and profile directory. Stop events registered when a run is claimed/created (queued cancel works).

## AI pipeline

`transcribe_recording` → `clean_transcript` → `generate_meeting_notes` via DB jobs (`FOR UPDATE SKIP LOCKED`). Uses **OpenAI** (`/v1` audio transcriptions + chat completions). Action items stored in first-class `action_items` table.
