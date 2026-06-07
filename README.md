# AI Meeting Notetaker

Turborepo monorepo for an AI-powered Google Meet notetaker: Rust backend (gRPC + OAuth) and React frontend.

Planned work: [docs/ROADMAP.md](docs/ROADMAP.md).

## Layout

```
.
├── apps/
│   ├── backend/      # Rust API, bot, scheduler, SQLite
│   └── frontend/     # React + Vite UI (grpc-web)
├── packages/
│   └── proto/        # `.proto` sources + generated TS (`@ai-meeting-notetaker/proto`)
├── docs/
│   └── ROADMAP.md    # Forward-looking plan
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
| `bun run proto:generate` | Regenerate TS clients in `packages/proto` (`@ai-meeting-notetaker/proto`) |
| `bun run db:reset` | Reset SQLite via backend CLI |
| `bun run test` | Run backend tests |

## Configuration

- **`.env`** (`apps/backend/.env`) — secrets: Google OAuth, AWS/S3, Groq, optional `GMR_STORAGE__TOKEN_ENCRYPTION_KEY`
- **`apps/backend/config.toml`** — server ports, bot paths, storage

Paths in `apps/backend/config.toml` are relative to `apps/backend/`. Default `data_dir = "../../data"` keeps SQLite and Chrome profile at the monorepo root.

### OAuth and CORS

Google OAuth uses **two URLs**:

| URL | Purpose |
|-----|---------|
| `GMR_GOOGLE__REDIRECT_URI` (default `http://localhost:8080/auth/google/callback`) | Backend HTTP handler — register this in Google Cloud Console |
| `GMR_GOOGLE__FRONTEND_REDIRECT_URL` (default `http://localhost:5173/app/calendars`) | Browser redirect after OAuth completes |

grpc-web CORS (`GMR_GRPC__ALLOW_ORIGIN`) accepts a **comma-separated** list of frontend origins. Default includes both Vite dev (`:5173`) and Docker (`:3000`).

If migrations fail at startup with an incompatible schema, run `bun run db:reset` (or `cargo run -- db reset`). Automatic recreation is opt-in via `GMR_STORAGE__AUTO_MIGRATE_RESET=true`.

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

On a headless server, use the **debug** image with RustDesk to see Chrome on the bot's virtual display (`:99`):

```bash
cp debug.env.example .env   # set RUSTDESK_ID_SERVER, RUSTDESK_KEY, RUSTDESK_PASSWORD

docker compose --profile debug run --rm --build backend-debug profile
```

The container registers with your **self-hosted RustDesk server** (`hbbs`/`hbbr`), prints a **RustDesk ID**, and waits `DEBUG_WAIT_SECS` (default 15s). Connect from the [RustDesk client](https://rustdesk.com/) on your laptop — your client must use the **same ID server and key** (Settings → Network). Enter the container password, then Chrome opens on display `:99`. Sign into Google and close Chrome.

To run the full app while watching UI/scheduler-triggered Chrome windows in RustDesk, swap the main `backend` service to the debug image:

```bash
docker compose -f docker-compose.yml -f docker-compose.debug.yml up --build
```

Use this override when testing recordings from the frontend. `docker compose --profile debug up` starts a separate `backend-debug` helper container, but the frontend still talks to the normal `backend` service, so Chrome from scheduled runs will not appear in that helper's RustDesk desktop.

With a self-hosted server, the debug container does **not** need inbound ports `21115–21117` on AWS — only your RustDesk server does. The bot container connects outbound to `hbbs`.

Without RustDesk (local machine with a display):

```bash
docker compose run --rm --service-ports backend profile
```

Other CLI commands:

```bash
docker compose run --rm backend db reset
docker compose run --rm backend profile-status
```

**Debug** (interactive shell on display `:99`, RustDesk stays up):

```bash
docker compose --profile debug run --rm --build -it backend-debug
# inside the container: google-meeting-recorder profile
# or: google-meeting-recorder join 'https://meet.google.com/...'
```

Optional debug env (repo-root `.env`; see `debug.env.example`):

```bash
RUSTDESK_PASSWORD=ChooseAStrongPassword
RUSTDESK_ID_SERVER=rustdesk.example.com    # hbbs
RUSTDESK_KEY=<contents of id_ed25519.pub>
RUSTDESK_RELAY_SERVER=rustdesk.example.com # hbbr (optional)
RUSTDESK_CONFIG=<base64 export>            # alternative to ID server + key
DEBUG_WAIT_SECS=20
DEBUG_X11_DISPLAY=:99
```

Optional compose env (repo-root `.env` or shell):

```bash
PUBLIC_FRONTEND_URL=http://localhost:3000
PUBLIC_OAUTH_CALLBACK_URL=http://localhost:8080/auth/google/callback
FRONTEND_PORT=3000
BACKEND_HTTP_PORT=8080
```
