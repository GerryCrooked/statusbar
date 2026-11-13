#!/usr/bin/env bash
set -Eeuo pipefail
SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${1:-}"
if [[ -z "$DEST" || ! -f "$DEST/.env" || ! -f "$DEST/server/src/main.rs" ]]; then
  echo "Usage: bash scripts/upgrade-v043.sh /opt/docker/vg-statusbar" >&2;exit 1
fi
DEST="$(cd "$DEST" && pwd)"
[[ "$SRC" != "$DEST" ]] || { echo "Do not upgrade in-place from the source directory" >&2;exit 1; }
# Fail early if a mounted archive has unexpected content.
for f in server/src/main.rs server/Cargo.toml server/static/index.html client/src/main.rs client/Cargo.toml scripts/build-v043.sh docs/DISPLAY-CONFIG.md .env.example README.md; do
  [[ -f "$SRC/$f" ]] || { echo "Missing source $f" >&2;exit 1; }
done
grep -q 'rdp_compat_mode' "$SRC/client/src/main.rs" || { echo 'Bad source: RDP code missing' >&2;exit 1; }
grep -q 'auth_mode' "$SRC/server/src/main.rs" || { echo 'Bad source: standalone auth missing' >&2;exit 1; }
BACKUP="$DEST/backups/source-before-v043-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$BACKUP"
for f in server/src/main.rs server/Cargo.toml server/static/index.html client/src/main.rs client/Cargo.toml scripts/build-v043.sh docs/DISPLAY-CONFIG.md .env.example README.md; do
  mkdir -p "$BACKUP/$(dirname "$f")" "$DEST/$(dirname "$f")"
  if [[ -f "$DEST/$f" ]];then cp -p "$DEST/$f" "$BACKUP/$f";fi
  cp "$SRC/$f" "$DEST/$f"
done
if ! grep -q '^BAR_RDP_COMPAT_MODE=' "$DEST/.env";then printf '\nBAR_RDP_COMPAT_MODE=false\n' >> "$DEST/.env";fi
chmod 600 "$DEST/.env" || true
printf '\nUpdated v0.4.3 sources at: %s\nBackup: %s\n' "$DEST" "$BACKUP"
printf 'The running server, tokens, private CA data and SQLite database were NOT modified.\n'
printf 'Check .env and rebuild server with: cd %q && docker compose build status-server && docker compose up -d --force-recreate status-server\n' "$DEST"
printf 'For the Windows client: bash scripts/build-v043.sh\n'
