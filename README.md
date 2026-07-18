# MeetOps — AWS VPS Deployment

Runbook for deploying MeetOps on `ubuntu@aws-meetops-vps`. This folder lives **outside the git repo** — it is synced to the server with `scripts/push-to-vps.sh`, not `git pull`.

Architecture on the VPS:

- **Images** are built by GitHub Actions (`.github/workflows/docker-images.yml` in the repo) and pushed to ECR. The VPS only pulls.
- **Postgres** is AWS RDS (`DATABASE_URL` in `.env`). No database container.
- **Ingress** is Tailscale: `tailscale serve` terminates HTTPS on the tailnet name and proxies to the `web` container on `127.0.0.1:3000`. Caddy inside `web` serves the SPA and proxies `/api/*` and `/auth/*` to `api`. Nothing is exposed publicly.
- **Data** (`./data`): bot Chrome profiles and staging recordings. Back this up if you care about not re-bootstrapping the bot profile.

## Prerequisites

- VPS joined to your tailnet (`sudo tailscale up`).
- IAM: instance role (or `aws configure`) allowing ECR pull; S3 read/write for the recordings bucket (or set keys in `.env`).
- RDS security group allows Postgres from the VPS; database `meetops` exists (migrations run automatically on api start).
- S3 bucket created, with CORS for playback (see `core/docs/OPERATIONS.md`).
- Google OAuth client with redirect URI `https://<vps>.<tailnet>.ts.net/auth/google/callback`.

## First deploy

```bash
# Local machine — sync this folder to the server
scripts/push-to-vps.sh                 # rsyncs to ubuntu@aws-meetops-vps:~/meetops/

# On the VPS
ssh ubuntu@aws-meetops-vps
cd ~/meetops
cp .env.example .env && vim .env       # fill in secrets, tailnet name, RDS URL, ECR registry
scripts/setup-vps.sh                   # docker, aws cli, tailscale serve (one-time)
scripts/deploy.sh                      # ECR login → pull → up
```

Then bootstrap the bot Google profile (one-time):

```bash
# On the VPS
scripts/debug-up.sh                    # swaps worker for worker-debug (noVNC)

# From your machine
ssh -L 6080:localhost:6080 ubuntu@aws-meetops-vps
# open http://localhost:6080, then on the VPS:
docker compose -f docker-compose.yml -f docker-compose.debug.yml --profile debug \
  exec worker-debug meetops-cli bot profile-bootstrap
# sign into the dedicated bot Google account in the noVNC window

# On the VPS, when done
scripts/debug-down.sh                  # restores the regular worker
```

## Routine deploys

CI pushes `:latest` and `:<12-char-sha>` tags on every merge to `main`. On the VPS:

```bash
scripts/deploy.sh            # deploy IMAGE_TAG from .env (default: latest)
scripts/deploy.sh 1a2b3c4d5e6f   # or pin a specific commit SHA (rollback works the same way)
```

## Scripts

| Script | Where | What |
|---|---|---|
| `push-to-vps.sh [user@host]` | local | rsync this folder to `~/meetops` on the server (skips `.env`, `data/`) |
| `setup-vps.sh` | VPS | one-time: Docker, AWS CLI, data dir, `tailscale serve` |
| `deploy.sh [tag]` | VPS | ECR login, pull, `up -d`, prune |
| `prod-up.sh` / `prod-down.sh` | VPS | start/stop the stack without pulling |
| `debug-up.sh` / `debug-down.sh` | VPS | swap in/out the noVNC debug worker for bot bootstrap |
| `ecr-login.sh` | VPS | just the ECR docker login |

## Health & troubleshooting

```bash
docker compose ps
docker compose logs -f api worker
curl -sf http://127.0.0.1:3000/                              # SPA via Caddy
docker compose exec api curl -sf http://localhost:8000/healthz   # API health
docker compose exec worker meetops-cli bot profile-status
tailscale serve status
```

- App URL: `https://<vps>.<tailnet>.ts.net`
- Worker shutdown is graceful (SIGTERM → finish uploads, ≤120s) — `prod-down.sh` uses a 150s timeout.
- If bot joins start failing with session errors, re-run the bootstrap flow above (profile health is also checked every 15 min and surfaced in the UI).
