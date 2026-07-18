#!/usr/bin/env bash
# Start the debug worker (noVNC) for the one-time Google bot profile bootstrap.
# Replaces the regular worker while running.
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${DEPLOY_DIR}"

docker compose stop worker || true
docker compose -f docker-compose.yml -f docker-compose.debug.yml --profile debug up -d worker-debug

cat <<'EOF'

Debug worker is up. To bootstrap the bot Google profile:

  1. From your machine:   ssh -L 6080:localhost:6080 ubuntu@aws-meetops-vps
  2. Open:                http://localhost:6080  (noVNC)
  3. On the VPS:          docker compose -f docker-compose.yml -f docker-compose.debug.yml --profile debug \
                            exec worker-debug meetops-cli bot profile-bootstrap
  4. Sign into the dedicated bot Google account in the noVNC browser window.
  5. When done:           scripts/debug-down.sh

Profiles persist in ./data/chrome-profiles.
EOF
