# AI Meeting Notetaker

Turborepo monorepo for an AI-powered Google Meet notetaker: Rust backend (gRPC + OAuth) and React frontend.

## Layout

```
.
├── apps/
│   ├── backend/      # Rust API, bot, scheduler, SQLite
│   └── frontend/     # React + Vite UI (grpc-web)
├── packages/
│   └── proto/        # Shared protobuf contracts
├── data/             # Local runtime data (gitignored)
├── turbo.json
└── package.json      # Bun workspaces + Turbo scripts
```

## Prerequisites

- [Bun](https://bun.sh) (workspace + Turbo)
- Rust toolchain
- Google Chrome, Xvfb, xdotool (Linux bot recording)
- S3 bucket + AWS credentials (recordings)

## Quick start

```bash
bun install

cp apps/backend/.env.example apps/backend/.env
cp apps/backend/config.toml.example apps/backend/config.toml

# Run backend + frontend together
bun run dev
```

Or run individually:

```bash
# Backend (gRPC :50051, OAuth HTTP :8080)
cd apps/backend && cargo run

# Frontend (React :5173)
cd apps/frontend && bun dev
```

## Turbo scripts

| Command | Description |
|---------|-------------|
| `bun run dev` | Start all `dev` tasks (backend + frontend) |
| `bun run build` | Build frontend (runs `proto:generate` first) |
| `bun run proto:generate` | Regenerate TS clients from `packages/proto` |
| `bun run db:reset` | Reset SQLite via backend CLI |
| `bun run test` | Run backend tests |

## Configuration

- **`.env`** (`apps/backend/.env`) — secrets: Google OAuth, AWS/S3, Groq
- **`apps/backend/config.toml`** — server ports, bot paths, storage

Paths in `apps/backend/config.toml` are relative to `apps/backend/`. Default `data_dir = "../../data"` keeps SQLite and Chrome profile at the monorepo root.

## Docker (recommended for the recording bot)

The backend image includes Google Chrome, Xvfb, xdotool, openbox, ImageMagick, and ffmpeg — no host install required.

```bash
cp apps/backend/.env.docker.example apps/backend/.env
# Fill in Google OAuth + S3 credentials in apps/backend/.env

docker compose up --build
```

| URL | Service |
|-----|---------|
| http://localhost:3000 | React UI (grpc-web proxied to backend) |
| http://localhost:8080 | OAuth callback (`/auth/google/callback`) |

Register `http://localhost:8080/auth/google/callback` in Google Cloud Console.

**One-time bot Google sign-in** (persists in the repo `data/` directory, shared with local dev):

```bash
docker compose run --rm --service-ports backend profile
```

Other CLI commands:

```bash
docker compose run --rm backend db reset
docker compose run --rm backend profile-status
```

Optional compose env (repo-root `.env` or shell):

```bash
PUBLIC_FRONTEND_URL=http://localhost:3000
PUBLIC_OAUTH_CALLBACK_URL=http://localhost:8080/auth/google/callback
FRONTEND_PORT=3000
BACKEND_HTTP_PORT=8080
```
