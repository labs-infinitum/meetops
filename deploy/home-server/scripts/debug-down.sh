#!/usr/bin/env bash
# Remove the noVNC worker and start the normal one again.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
docker compose -f compose.yml -f compose.debug.yml --profile debug rm -sf worker-debug
docker compose up -d worker
docker compose ps
