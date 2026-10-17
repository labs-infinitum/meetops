#!/usr/bin/env bash
# Set one value in .env from a hidden prompt, e.g.: set-secret.sh OPENAI_API_KEY
# Restart afterwards (scripts/deploy.sh, or docker compose up -d) so the containers pick it up.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
name="${1:?usage: set-secret.sh NAME}"
[[ "$name" =~ ^[A-Z0-9_]+$ ]] || { echo "bad name" >&2; exit 2; }
read -r -s -p "$name: " value; echo
[ -n "$value" ] || { echo "empty, nothing changed"; exit 1; }
python3 - "$name" "$value" <<'PY'
import os, sys
name, value = sys.argv[1], sys.argv[2]
lines = open(".env").read().splitlines()
out, done = [], False
for l in lines:
    if l.split("=", 1)[0] == name:
        out.append(f"{name}={value}"); done = True
    else:
        out.append(l)
if not done:
    out.append(f"{name}={value}")
tmp = ".env.tmp"
with open(os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600), "w") as f:
    f.write("\n".join(out) + "\n")
os.replace(tmp, ".env")
PY
echo "saved $name"
