#!/usr/bin/env bash
# One-command pilot pipeline: clone public source, overlay v0.4.2 client only, build, export private config.
set -Eeuo pipefail
umask 077
KIT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LIVE="${STATUSBAR_LIVE:-/opt/docker/vg-statusbar}"
CHECKOUT="${STATUSBAR_BUILD_SOURCE:-/opt/docker/statusbar-build-source/statusbar}"
command -v git >/dev/null || { echo 'git required' >&2;exit 1; }
if [[ ! -d "$CHECKOUT/.git" ]]; then
  [[ ! -e "$CHECKOUT" ]] || { echo "STOP: checkout path exists but is not a Git clone: $CHECKOUT" >&2;exit 1; }
  mkdir -p "$(dirname "$CHECKOUT")"
  git clone https://github.com/GerryCrooked/statusbar.git "$CHECKOUT"
else
  remote="$(git -C "$CHECKOUT" remote get-url origin)"
  [[ "$remote" == 'https://github.com/GerryCrooked/statusbar.git' || "$remote" == 'https://github.com/GerryCrooked/statusbar' || "$remote" == 'git@github.com:GerryCrooked/statusbar.git' ]] || {
    echo "STOP: unexpected origin remote: $remote" >&2;exit 1;
  }
fi
bash "$KIT/linux/sync-client-sources.sh" "$LIVE" "$CHECKOUT"
bash "$KIT/linux/build-local.sh" "$CHECKOUT"
bash "$KIT/linux/export-client-config.sh" "$LIVE/.env"
echo
printf 'DONE. Unsigned binary: %s\n' "$KIT/dist/unsigned/vg-status-client.exe"
printf 'PRIVATE client config: %s\n' "$KIT/dist/private/client.json"
echo 'Next: transfer to authorized Windows signing station.'
