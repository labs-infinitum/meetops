#!/bin/sh
set -e
echo "Running migrations…"
uv run alembic upgrade head
echo "Starting API…"
exec uv run meetops-api
