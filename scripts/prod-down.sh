#!/usr/bin/env bash
# Stop the production stack. Worker shuts down gracefully (finishes uploads, ≤120s).
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${DEPLOY_DIR}"

docker compose down --timeout 150
