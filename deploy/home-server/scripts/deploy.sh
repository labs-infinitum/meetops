#!/usr/bin/env bash
# Build from this checkout and (re)start the stack. Usage: deploy.sh [git-ref]
# With a ref, checks it out first (e.g. a commit to roll back to); otherwise pulls the current branch.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
if [ $# -ge 1 ]; then
  git fetch -q origin && git checkout -q "$1"
else
  git pull -q --ff-only
fi
echo "deploying $(git rev-parse --short HEAD)"
docker compose build --pull
docker compose up -d --remove-orphans
docker image prune -f >/dev/null
docker compose ps
