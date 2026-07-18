#!/usr/bin/env bash
# Stop the debug worker and restore the regular worker.
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${DEPLOY_DIR}"

docker compose -f docker-compose.yml -f docker-compose.debug.yml --profile debug stop worker-debug
docker compose -f docker-compose.yml -f docker-compose.debug.yml --profile debug rm -f worker-debug
docker compose up -d worker
docker compose ps
