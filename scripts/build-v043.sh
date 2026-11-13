#!/usr/bin/env bash
set -Eeuo pipefail
KIT="${STATUSBAR_BUILD_KIT:-/opt/docker/vg-statusbar-local-build-kit}"
[[ -f "$KIT/linux/prepare-release.sh" ]] || { echo "Build kit not found: $KIT" >&2;exit 1; }
[[ -f /opt/docker/vg-statusbar/client/src/main.rs ]] || { echo 'Expected deployed client at /opt/docker/vg-statusbar' >&2;exit 1; }
grep -q 'rdp_compat_mode' /opt/docker/vg-statusbar/client/src/main.rs || { echo 'Installed source is not v0.4.3' >&2;exit 1; }
cd "$KIT"
bash linux/prepare-release.sh
printf 'Unsigned EXE (keep secrets private): %s/dist/unsigned/vg-status-client.exe\n' "$KIT"
