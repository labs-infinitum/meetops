#!/bin/sh
set -e
echo "Running migrations…"
uv run --no-sync alembic upgrade head
echo "Starting API…"
exec uv run --no-sync meetops-api
