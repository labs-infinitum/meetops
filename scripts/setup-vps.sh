#!/usr/bin/env bash
# One-time VPS preparation (idempotent). Run on the server as the ubuntu user.
# Installs Docker + compose plugin and AWS CLI v2, creates the data dir,
# and configures tailscale serve to front the web container.
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# --- Docker Engine + compose plugin ---
if ! command -v docker >/dev/null 2>&1; then
  echo "Installing Docker…"
  curl -fsSL https://get.docker.com | sudo sh
else
  echo "Docker already installed: $(docker --version)"
fi

if ! id -nG "$USER" | grep -qw docker; then
  sudo usermod -aG docker "$USER"
  echo "Added $USER to the docker group — log out/in (or run 'newgrp docker') to apply."
fi

# --- AWS CLI v2 (for ECR login) ---
if ! command -v aws >/dev/null 2>&1; then
  echo "Installing AWS CLI v2…"
  tmp="$(mktemp -d)"
  arch="$(uname -m)"  # x86_64 or aarch64
  curl -fsSL "https://awscli.amazonaws.com/awscli-exe-linux-${arch}.zip" -o "${tmp}/awscliv2.zip"
  unzip -q "${tmp}/awscliv2.zip" -d "${tmp}"
  sudo "${tmp}/aws/install" --update
  rm -rf "${tmp}"
else
  echo "AWS CLI already installed: $(aws --version)"
fi

# --- Data directory (chrome profiles, staging recordings) ---
mkdir -p "${DEPLOY_DIR}/data"

# --- Tailscale serve: HTTPS on the tailnet → web container ---
if command -v tailscale >/dev/null 2>&1; then
  if tailscale status >/dev/null 2>&1; then
    echo "Configuring tailscale serve (https :443 → http://127.0.0.1:3000)…"
    sudo tailscale serve --bg --https=443 http://127.0.0.1:3000
    tailscale serve status || true
  else
    echo "Tailscale is installed but not up — run 'sudo tailscale up' first, then re-run this script."
  fi
else
  echo "Tailscale not installed — install it (https://tailscale.com/download/linux),"
  echo "run 'sudo tailscale up', then re-run this script."
fi

echo
echo "Setup done. Next: cp .env.example .env, fill it in, then scripts/deploy.sh"
