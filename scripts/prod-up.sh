#!/usr/bin/env bash
# Start the production stack (without pulling — use deploy.sh to update images).
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${DEPLOY_DIR}"

docker compose up -d --remove-orphans
docker compose ps
