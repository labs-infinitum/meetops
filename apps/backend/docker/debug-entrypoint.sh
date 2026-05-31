#!/bin/bash
set -euo pipefail

RECORDER="/usr/local/bin/google-meeting-recorder"

display_num="${DEBUG_X11_DISPLAY:-:99}"
display_num="${display_num#:}"
display=":${display_num}"
export DISPLAY="$display"

export GMR_BOT__X11_DISPLAY="$display"
export GMR_BOT__X11_AUTO_VIRTUAL="${GMR_BOT__X11_AUTO_VIRTUAL:-false}"

xvfb_screen="${DEBUG_XVFB_SCREEN:-1920x1080x24}"
wait_secs="${DEBUG_WAIT_SECS:-15}"

echo "[debug] Starting Xvfb on ${display}..."
Xvfb "$display" -screen 0 "$xvfb_screen" -nolisten tcp -ac &
XVFB_PID=$!

for i in $(seq 1 30); do
  if xdpyinfo -display "$display" >/dev/null 2>&1; then
    break
  fi
  if [ "$i" -eq 30 ]; then
    echo "[debug] Xvfb failed to start on ${display}" >&2
    exit 1
  fi
  sleep 0.5
done
echo "[debug] Xvfb ready on ${display}"

echo "[debug] Starting openbox..."
openbox &
OPENBOX_PID=$!
sleep 1

if command -v dbus-launch >/dev/null; then
  eval "$(dbus-launch --sh-syntax)"
  export DBUS_SESSION_BUS_ADDRESS
fi

# Self-hosted hbbs/hbbr (see debug.env.example).
/usr/local/bin/configure-rustdesk-server.sh

if [ -n "${RUSTDESK_PASSWORD:-}" ]; then
  echo "[debug] Setting RustDesk password..."
  mkdir -p "${HOME}/.config/rustdesk"
  rustdesk --password "$RUSTDESK_PASSWORD" || echo "[debug] WARNING: rustdesk --password failed (continuing)" >&2
fi

echo "[debug] Starting RustDesk..."
export LIBGL_ALWAYS_SOFTWARE=1
rustdesk &
RUSTDESK_PID=$!

for i in $(seq 1 10); do
  if kill -0 "$RUSTDESK_PID" 2>/dev/null; then
    break
  fi
  sleep 0.5
done
if ! kill -0 "$RUSTDESK_PID" 2>/dev/null; then
  echo "[debug] WARNING: RustDesk exited early — check libEGL/Mesa packages in the debug image" >&2
  RUSTDESK_PID=
fi
sleep 2

rustdesk_id="$(rustdesk --get-id 2>/dev/null || echo "unknown")"
rustdesk_server="${RUSTDESK_ID_SERVER:-public RustDesk}"
cat <<EOF

========================================
 RustDesk remote desktop (debug)
========================================
 ID:       ${rustdesk_id}
 Server:   ${rustdesk_server}
 Display:  ${display}
 Password: (RUSTDESK_PASSWORD env var)
========================================
 Your laptop RustDesk client must use the
 same ID server + key as this container.
 Chrome and bot commands use ${display}.
========================================

EOF

cleanup() {
  if [ -n "${RUSTDESK_PID:-}" ]; then
    kill "$RUSTDESK_PID" 2>/dev/null || true
  fi
  kill "$OPENBOX_PID" 2>/dev/null || true
  kill "$XVFB_PID" 2>/dev/null || true
}
trap cleanup EXIT

if [ "$#" -gt 0 ] && [ "$wait_secs" -gt 0 ] 2>/dev/null; then
  echo "[debug] Waiting ${wait_secs}s for RustDesk connection before: $*"
  sleep "$wait_secs"
fi

if [ "$#" -eq 0 ]; then
  echo "[debug] No command — starting interactive shell."
  exec bash
fi

exec "$RECORDER" "$@"
