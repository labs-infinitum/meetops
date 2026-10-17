#!/usr/bin/env bash
# One-time setup on the server (Ubuntu 24.04, run as the normal user with sudo):
# Docker, data folder, .env with generated secrets, tailscale serve, nightly database backups.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
HERE="$(pwd)"

if ! command -v docker >/dev/null || ! docker compose version >/dev/null 2>&1; then
  sudo apt-get update -qq
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq docker.io docker-compose-v2 >/dev/null
fi
id -nG "$USER" | grep -qw docker || { sudo usermod -aG docker "$USER"; echo "added $USER to the docker group (log in again)"; }

# the app containers run as uid 1000
mkdir -p data/chrome-profiles data/recordings data/backups
[ "$(id -u)" = 1000 ] || sudo chown -R 1000:1000 data

TS_NAME="$(tailscale status --json 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["Self"]["DNSName"].rstrip("."))' || true)"
if [ ! -f .env ]; then
  [ -n "$TS_NAME" ] || { echo "join the tailnet first (sudo tailscale up)" >&2; exit 1; }
  rand() { python3 -c 'import secrets; print(secrets.token_urlsafe(24))'; }
  dbpw="$(rand)"
  fernet="$(python3 -c 'import base64,os; print(base64.urlsafe_b64encode(os.urandom(32)).decode())')"
  sed -e "s|meetops.<tailnet>.ts.net|$TS_NAME|g" \
      -e "s|^POSTGRES_PASSWORD=.*|POSTGRES_PASSWORD=$dbpw|" \
      -e "s|<POSTGRES_PASSWORD>|$dbpw|" \
      -e "s|^MEETOPS_TOKEN_ENCRYPTION_KEY=.*|MEETOPS_TOKEN_ENCRYPTION_KEY=$fernet|" \
      -e "s|^MEETOPS_VNC_PASSWORD=.*|MEETOPS_VNC_PASSWORD=$(rand)|" .env.example > .env
  chmod 600 .env
  echo "wrote .env for https://$TS_NAME (generated database, encryption and noVNC secrets)"
fi

sudo tailscale serve --bg --https=443 http://127.0.0.1:3000 >/dev/null
echo "tailscale serve: https://$TS_NAME -> 127.0.0.1:3000"

sudo tee /etc/systemd/system/meetops-backup.service >/dev/null <<UNIT
[Unit]
Description=MeetOps nightly database dump
[Service]
Type=oneshot
User=$USER
ExecStart=$HERE/scripts/backup.sh
UNIT
sudo tee /etc/systemd/system/meetops-backup.timer >/dev/null <<'UNIT'
[Unit]
Description=MeetOps nightly database dump
[Timer]
OnCalendar=*-*-* 03:30
RandomizedDelaySec=15m
Persistent=true
[Install]
WantedBy=timers.target
UNIT
sudo systemctl daemon-reload
sudo systemctl enable --now meetops-backup.timer >/dev/null
echo "nightly database dumps: data/backups (meetops-backup.timer)"
echo "next: add API keys (scripts/set-secret.sh GOOGLE_CLIENT_ID ...), then scripts/deploy.sh"
