#!/bin/sh
# Konsistentes Backup der SQLite-Datenbank und der Exporte aus dem laufenden Container.
# Aufruf: deploy/backup.sh /pfad/zum/backupordner   (z. B. per Cron täglich)
set -eu
DEST="${1:-./backups}"
STAMP="$(date +%Y%m%d-%H%M%S)"
mkdir -p "$DEST"
CONTAINER="$(docker compose -f "$(dirname "$0")/compose.yml" ps -q timecard)"
[ -n "$CONTAINER" ] || { echo "Container nicht gefunden"; exit 1; }
# Online-Backup der DB (WAL-sicher) und Tar der Exporte
docker exec "$CONTAINER" sqlite3 /data/timecard.sqlite ".backup /data/backup-$STAMP.sqlite"
docker exec "$CONTAINER" tar -C /data -czf "/data/backup-$STAMP.tar.gz" "backup-$STAMP.sqlite" exports
docker cp "$CONTAINER:/data/backup-$STAMP.tar.gz" "$DEST/timecard-backup-$STAMP.tar.gz"
docker exec "$CONTAINER" rm -f "/data/backup-$STAMP.sqlite" "/data/backup-$STAMP.tar.gz"
# Nur die letzten 30 Backups behalten
ls -1t "$DEST"/timecard-backup-*.tar.gz 2>/dev/null | tail -n +31 | xargs -r rm -f
echo "Backup: $DEST/timecard-backup-$STAMP.tar.gz"
