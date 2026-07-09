# MeetOps-py

Self-hosted **meeting operations** platform: Google Calendar sync, Meet bot recording, AI notes & action items.

**Brand:** MeetOps / MeetOps.Pro

## STATUS

| Area | Status |
|------|--------|
| Auth (cookie sessions, email verify, reset) | Shipped |
| Settings API + UI | Shipped |
| Google Calendar OAuth + sync | Shipped |
| Meetings + dashboard | Shipped |
| Bot recording (patchright, concurrency N) | Shipped |
| S3 multipart upload | Shipped |
| AI notes + persisted todos | Shipped |
| Outlook / Apple calendars | Coming soon (UI disabled) |
| Live bot video preview | Not in v1 |
| Slack / Linear / billing / SSO | Not in v1 |

## Quickstart

```bash
cp .env.example .env
# Edit .env — at minimum set MEETOPS_ALLOW_PLAINTEXT_TOKENS=1 for local dev

# Python deps
uv sync

# Database (Compose maps host 5433 → container 5432 to avoid clashing with local Postgres)
docker compose up -d postgres
uv run alembic upgrade head

# API + worker (separate terminals)
uv run meetops-api
uv run meetops-worker

# Frontend
cd apps/web && bun install && bun run dev
```

Or full stack:

```bash
cp .env.example .env
docker compose up --build
# Web: http://localhost:3000  API: http://localhost:8000/docs
```

### Bot Google session bootstrap

Calendar OAuth ≠ Meet bot identity. Sign a dedicated Google account into the bot Chrome profile once:

```bash
docker compose -f docker-compose.yml -f docker-compose.debug.yml --profile debug up worker-debug
# Open http://localhost:6080 (noVNC)
uv run meetops-cli bot profile-bootstrap
```

## Commands

| Command | Description |
|---------|-------------|
| `uv run meetops-api` | Start API |
| `uv run meetops-worker` | Start worker (scheduler, bot, AI) |
| `uv run alembic upgrade head` | Migrate DB |
| `uv run meetops-cli db reset` | Drop+migrate (dev only) |
| `uv run meetops-cli bot profile-bootstrap` | Sign into Google for bot |
| `uv run meetops-cli bot profile-status` | Check profiles |
| `make test` / `make lint` | Tests and lint |

## Stack

Python 3.12 · FastAPI · PostgreSQL · patchright · S3 · OpenAI · React/Vite/bun · Docker Compose

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and [docs/OPERATIONS.md](docs/OPERATIONS.md).
