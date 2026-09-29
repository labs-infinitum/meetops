#!/bin/sh
set -e
mkdir -p /data/chrome-profiles /data/recordings/staging
Xvfb :99 -screen 0 1920x1080x24 -ac +extension RANDR &
sleep 1
DISPLAY=:99 openbox &
. /app/scripts/start-audio.sh
# VNC listens on container loopback only; websockify is the only way in.
if [ -n "${MEETOPS_VNC_PASSWORD:-}" ]; then
  x11vnc -storepasswd "$MEETOPS_VNC_PASSWORD" /tmp/vncpasswd >/dev/null
  chmod 600 /tmp/vncpasswd
  VNC_AUTH="-rfbauth /tmp/vncpasswd"
else
  echo "WARNING: MEETOPS_VNC_PASSWORD is not set — noVNC has no password"
  VNC_AUTH="-nopw"
fi
x11vnc -display :99 -forever -shared -localhost $VNC_AUTH -rfbport 5900 &
websockify --web /usr/share/novnc 6080 localhost:5900 &
echo "noVNC available on :6080 — run: uv run meetops-cli bot profile-bootstrap"
exec uv run meetops-worker
