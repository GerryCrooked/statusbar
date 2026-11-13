#!/usr/bin/env bash
set -euo pipefail
SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${1:-}"
if [[ -z "$DEST" || ! -d "$DEST/server/src" || ! -f "$DEST/.env" ]]; then
  echo 'Usage: bash scripts/upgrade-v042.sh /path/to/installed/vg-statusbar' >&2
  exit 1
fi
DEST="$(cd "$DEST" && pwd)"
if [[ "$SRC" == "$DEST" ]]; then echo 'Source and destination must differ.' >&2;exit 1;fi
BACKUP="$DEST/backups/source-before-v042-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$BACKUP/server/src" "$BACKUP/client/src" "$BACKUP/server"
for f in server/src/main.rs server/Cargo.toml client/src/main.rs client/Cargo.toml .env.example; do
  if [[ -f "$DEST/$f" ]];then mkdir -p "$BACKUP/$(dirname "$f")";cp "$DEST/$f" "$BACKUP/$f";fi
done
for f in server/src/main.rs server/Cargo.toml client/src/main.rs client/Cargo.toml .env.example docs/DISPLAY-CONFIG.md; do
  mkdir -p "$DEST/$(dirname "$f")"
  cp "$SRC/$f" "$DEST/$f"
done
for item in \
  'BAR_POSITION=bottom' \
  'BAR_ALL_MONITORS=true' \
  'BAR_HEIGHT_PX=44' \
  'BAR_INITIAL_HEIGHT_PX=88' \
  'BAR_INITIAL_SECONDS=5';do
  key="${item%%=*}"
  if ! grep -q "^${key}=" "$DEST/.env";then printf '\n%s\n' "$item" >> "$DEST/.env";fi
done
printf 'Updated sources in %s\nBackup: %s\nYour .env values, CA files and SQLite data have not been replaced.\n' "$DEST" "$BACKUP"
printf 'Next: review BAR_* in .env, compile on GitHub Actions, then rebuild the server and deploy the new Windows EXE.\n'
