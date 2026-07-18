#!/usr/bin/env bash
# Deploy MeetOps on the VPS: ECR login → pull → up → prune.
# Usage: deploy.sh [tag]   (tag overrides IMAGE_TAG from .env, e.g. a commit SHA)
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${DEPLOY_DIR}"

if [ $# -ge 1 ]; then
  export IMAGE_TAG="$1"
  echo "Deploying tag: ${IMAGE_TAG}"
fi

"${DEPLOY_DIR}/scripts/ecr-login.sh"

docker compose pull
docker compose up -d --remove-orphans
docker image prune -f

docker compose ps
