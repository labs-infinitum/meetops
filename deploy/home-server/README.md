# MeetOps on a home server

Runs the whole stack on one Linux machine (e.g. a Proxmox VM) for you and your team, reachable
only on your tailnet. Compared with the AWS deployment (`deployment` branch):

| | AWS | Home server |
|---|---|---|
| Images | CI → ECR, VPS pulls | built on the server from this checkout |
| Database | RDS | `postgres` container, nightly `pg_dump` to `data/backups` |
| Recordings | S3, presigned playback | `data/recordings/store`, streamed by the API to the owner (`MEETOPS_STORAGE_BACKEND=local`) |
| Ingress | `tailscale serve` | `tailscale serve` (same) |

## Machine

Ubuntu 24.04, **4 vCPU, 6 GB RAM, 40 GB disk** for `MEETOPS_BOT_CONCURRENCY=2` (each bot is a
full Chrome recording a call; add ~1.5 GB RAM and a core per extra bot). Use a VM, not an LXC
container: Docker and Chrome's sandbox need it. Grow the disk with your recordings
(roughly 0.2–1 GB per recorded hour, depending on the call).

## First install

```bash
sudo tailscale up --hostname meetops            # join the tailnet
git clone git@github.com:labs-infinitum/meetops.git ~/meetops
cd ~/meetops/deploy/home-server
scripts/setup.sh                                # docker, .env with generated secrets, tailscale serve, backups
scripts/set-secret.sh GOOGLE_CLIENT_ID          # hidden prompts; repeat for:
scripts/set-secret.sh GOOGLE_CLIENT_SECRET      #   DEEPGRAM_API_KEY, OPENAI_API_KEY (and RESEND_API_KEY if used)
scripts/deploy.sh                               # build + start (first build takes a few minutes)
```

In Google Cloud console add the redirect URI from `.env` (`GOOGLE_REDIRECT_URI`,
`https://<name>.<tailnet>.ts.net/auth/google/callback`) to the OAuth client.

Open `https://<name>.<tailnet>.ts.net` and sign up. Without Resend, leave
`MEETOPS_REQUIRE_EMAIL_VERIFICATION=false` (emails are skipped, so verification links never arrive).

## Bot sign-in (one time)

```bash
scripts/debug-up.sh
# from your computer:  ssh -L 6080:localhost:6080 <user>@<server>   then open http://localhost:6080
docker compose -f compose.yml -f compose.debug.yml --profile debug exec worker-debug meetops-cli bot profile-bootstrap
# sign into the dedicated bot Google account in the noVNC window, then:
scripts/debug-down.sh
```

The noVNC password is `MEETOPS_VNC_PASSWORD` in `.env`. The profile lives in `data/chrome-profiles`.

## Day to day

```bash
scripts/deploy.sh            # pull this branch, rebuild, restart
scripts/deploy.sh <commit>   # deploy (or roll back to) a specific commit
docker compose ps
docker compose logs -f api worker
docker compose exec worker meetops-cli bot profile-status
scripts/backup.sh            # database dump now (also runs nightly)
```

## Backups and restore

`data/` holds everything that isn't in Git: `.env` sits next to it, `data/backups` has the
database dumps, `data/recordings/store` the recordings, `data/chrome-profiles` the bot login.
Back up the VM (Proxmox) or at least these folders. Restore a dump into a fresh stack:

```bash
docker compose up -d postgres
docker compose exec -T postgres pg_restore -U meetops -d meetops --clean --if-exists < data/backups/<file>.dump
scripts/deploy.sh
```
