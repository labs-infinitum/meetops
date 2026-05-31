#!/bin/bash
# Configure RustDesk client for a self-hosted hbbs/hbbr before starting the daemon.
# Called from debug-entrypoint.sh.

set -euo pipefail

rustdesk_config_dir="${HOME}/.config/rustdesk"

configure_rustdesk_server() {
  mkdir -p "$rustdesk_config_dir"

  if [ -n "${RUSTDESK_CONFIG:-}" ]; then
    echo "[debug] Applying RustDesk server config from RUSTDESK_CONFIG..."
    rustdesk --config "$RUSTDESK_CONFIG" 2>/dev/null || true
    return 0
  fi

  if [ -z "${RUSTDESK_ID_SERVER:-}" ]; then
    echo "[debug] RUSTDESK_ID_SERVER unset — RustDesk will use public servers."
    echo "[debug] Set RUSTDESK_ID_SERVER + RUSTDESK_KEY for your self-hosted hbbs."
    return 0
  fi

  local id_server="$RUSTDESK_ID_SERVER"
  local relay_server="${RUSTDESK_RELAY_SERVER:-$id_server}"
  local key="${RUSTDESK_KEY:-}"
  local rendezvous="$id_server"

  if [[ "$rendezvous" != *:* ]]; then
    rendezvous="${rendezvous}:21116"
  fi

  echo "[debug] Writing RustDesk2.toml for self-hosted server (hbbs=${id_server})..."
  cat > "${rustdesk_config_dir}/RustDesk2.toml" <<EOF
rendezvous_server = '${rendezvous}'
nat_type = 1
serial = 0

[options]
custom-rendezvous-server = '${id_server}'
relay-server = '${relay_server}'
key = '${key}'
EOF

  if [ -n "${RUSTDESK_API_SERVER:-}" ]; then
    printf "api-server = '%s'\n" "$RUSTDESK_API_SERVER" >> "${rustdesk_config_dir}/RustDesk2.toml"
  fi

  if [ -z "$key" ]; then
    echo "[debug] WARNING: RUSTDESK_KEY is empty — copy the public key from hbbs id_ed25519.pub" >&2
  fi
}

configure_rustdesk_server
