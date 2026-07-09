#!/bin/sh
set -e
mkdir -p /data/chrome-profiles /data/recordings/staging
# Start a default Xvfb if not already present (slot allocator may start more)
if ! pgrep -x Xvfb >/dev/null 2>&1; then
  Xvfb :99 -screen 0 1920x1080x24 -ac +extension RANDR &
  sleep 1
  DISPLAY=:99 openbox &
fi
echo "Starting worker…"
exec uv run meetops-worker
