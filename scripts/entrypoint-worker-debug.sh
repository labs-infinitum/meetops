#!/bin/sh
set -e
mkdir -p /data/chrome-profiles /data/recordings/staging
Xvfb :99 -screen 0 1920x1080x24 -ac +extension RANDR &
sleep 1
DISPLAY=:99 openbox &
x11vnc -display :99 -forever -shared -nopw -rfbport 5900 &
websockify --web /usr/share/novnc 6080 localhost:5900 &
echo "noVNC available on :6080 — run: uv run meetops-cli bot profile-bootstrap"
exec uv run meetops-worker
