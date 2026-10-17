#!/usr/bin/env bash
# Swap in the noVNC worker for the one-time Google bot sign-in (see README.md).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
docker compose stop worker || true
docker compose -f compose.yml -f compose.debug.yml --profile debug up -d worker-debug
echo "noVNC on 127.0.0.1:6080 (password: MEETOPS_VNC_PASSWORD in .env). Then:"
echo "  docker compose -f compose.yml -f compose.debug.yml --profile debug exec worker-debug meetops-cli bot profile-bootstrap"
