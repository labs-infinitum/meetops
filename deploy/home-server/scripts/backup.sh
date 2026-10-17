#!/usr/bin/env bash
# Dump the database to data/backups (keeps the newest 14). Run nightly by meetops-backup.timer.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
out="data/backups/meetops-$(date +%Y%m%d-%H%M%S).dump"
docker compose exec -T postgres pg_dump -U meetops -d meetops -Fc > "$out.part"
mv "$out.part" "$out"
ls -1t data/backups/meetops-*.dump | tail -n +15 | xargs -r rm -f
echo "wrote $out"
