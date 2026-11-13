#!/bin/sh
set -eu
TARGET="${1:-/opt/docker/vg-statusbar}"
SOURCE=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
[ "$SOURCE" != "$TARGET" ] || { echo "Source and target must be different directories" >&2; exit 1; }
[ -f "$TARGET/compose.yaml" ] || { echo "No compose.yaml at $TARGET" >&2; exit 1; }
[ -f "$SOURCE/server/locales/en.json" ] || { echo "Release resources missing" >&2; exit 1; }
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
BACKUP="$TARGET/backups/source-before-v04-$STAMP"
mkdir -p "$BACKUP/server/src" "$BACKUP/server/static" "$BACKUP/server/locales"
for f in server/src/main.rs server/static/index.html server/Dockerfile server/Cargo.toml; do
  [ ! -f "$TARGET/$f" ] || cp "$TARGET/$f" "$BACKUP/$f" 2>/dev/null || {
    mkdir -p "$BACKUP/$(dirname "$f")";cp "$TARGET/$f" "$BACKUP/$f";
  }
done
[ ! -d "$TARGET/server/locales" ] || cp -a "$TARGET/server/locales/." "$BACKUP/server/locales/"
for f in server/src/main.rs server/static/index.html server/Dockerfile server/Cargo.toml; do
  cp "$SOURCE/$f" "$TARGET/$f"
done
mkdir -p "$TARGET/server/locales"
cp "$SOURCE/server/locales/en.json" "$SOURCE/server/locales/de.json" "$TARGET/server/locales/"
printf 'v0.4 source copied into %s. Backups: %s\n' "$TARGET" "$BACKUP"
printf 'Config, CA files, and SQLite volume have NOT been touched.\n'
printf 'Back up your Docker SQLite volume separately before rebuilding.\n'
