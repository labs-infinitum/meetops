#!/bin/sh
set -e
mkdir -p /data/chrome-profiles /data/recordings/staging
# Start a default Xvfb if not already present (slot allocator may start more)
if ! pgrep -x Xvfb >/dev/null 2>&1; then
  # A restarted container keeps /tmp: drop X locks/sockets from the previous run (this one and
  # the extra slots' displays), or Xvfb refuses to start ("Server is already active").
  rm -f /tmp/.X*-lock /tmp/.X11-unix/X*
  Xvfb :99 -screen 0 1920x1080x24 -ac +extension RANDR &
  sleep 1
  DISPLAY=:99 openbox &
fi
. /app/scripts/start-audio.sh
echo "Starting worker…"
exec uv run --no-sync meetops-worker
