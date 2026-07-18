#!/usr/bin/env bash
# Run LOCALLY: sync this deployment folder to the VPS (it is not in the git repo).
# Never touches the server's .env or data/.
# Usage: push-to-vps.sh [user@host]   (default: ubuntu@aws-meetops-vps)
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${1:-ubuntu@aws-meetops-vps}"

rsync -av --delete \
  --exclude '.env' \
  --exclude 'data/' \
  "${DEPLOY_DIR}/" "${TARGET}:~/meetops/"

echo "Synced to ${TARGET}:~/meetops/"
