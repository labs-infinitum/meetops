#!/usr/bin/env bash
# Dump the database to data/backups (keeps the newest 14). Run nightly by meetops-backup.timer.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
out="data/backups/meetops-$(date +%Y%m%d-%H%M%S).dump"
if docker compose ps --status running --services 2>/dev/null | grep -qx postgres; then
  docker compose exec -T postgres pg_dump -U meetops -d meetops -Fc > "$out.part"
else   # external server: dump through DATABASE_URL with a matching client
  url="$(grep -E '^DATABASE_URL=' .env | tail -1 | cut -d= -f2- | sed 's|+asyncpg||')"
  docker run --rm --network host postgres:17-alpine pg_dump -Fc "$url" > "$out.part"
fi
mv "$out.part" "$out"
ls -1t data/backups/meetops-*.dump | tail -n +15 | xargs -r rm -f
echo "wrote $out"
