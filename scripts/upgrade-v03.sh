#!/bin/sh
set -eu
# Run from inside the extracted release folder; source will not touch .env
TARGET="${1:-/opt/docker/vg-statusbar}"
[ -f "$TARGET/compose.yaml" ] || { echo "Target missing compose.yaml: $TARGET" >&2; exit 1; }
for f in server/src/main.rs server/Cargo.toml server/static/index.html; do
  [ -f "$f" ] || { echo "Run from extracted project root" >&2; exit 1; }
  [ -f "$TARGET/$f" ] || { echo "Missing target file: $TARGET/$f" >&2; exit 1; }
done
mkdir -p "$TARGET/backups"
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
for f in server/src/main.rs server/Cargo.toml server/static/index.html; do
  cp "$TARGET/$f" "$TARGET/backups/$(basename "$f").$STAMP"
  cp "$f" "$TARGET/$f"
done
printf 'Updated source in %s. No .env or CA certificates were copied.\n' "$TARGET"
printf 'Now back up the Docker SQLite volume, then run: cd %s && docker compose up -d --build --force-recreate\n' "$TARGET"
