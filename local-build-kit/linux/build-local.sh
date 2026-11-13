#!/usr/bin/env bash
set -Eeuo pipefail
umask 077
ROOT="${1:-/opt/docker/statusbar-build-source/statusbar}"
ROOT="$(realpath "$ROOT")"
[[ "${2:-}" == "--build-only" || -z "${2:-}" ]] || { echo "Unknown option: $2" >&2;exit 1; }
KIT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="${STATUSBAR_DIST:-$KIT_DIR/dist}"
IMAGE="${CARGO_XWIN_IMAGE:-messense/cargo-xwin:latest}"
mkdir -p "$DIST/unsigned" "$DIST/private" "$KIT_DIR/.cache/xwin"
chmod 700 "$DIST" "$DIST/unsigned" "$DIST/private" "$KIT_DIR/.cache/xwin"
for x in docker sha256sum file; do command -v "$x" >/dev/null || { echo "Missing dependency: $x" >&2; exit 1; }; done
[[ -f "$ROOT/client/Cargo.toml" && -f "$ROOT/client/src/main.rs" ]] || { echo "Client source not found: $ROOT" >&2; exit 1; }
[[ -d "$ROOT/.git" ]] || { echo 'STOP: build only from a separate Git checkout, not the live server tree' >&2;exit 1; }
if [[ -f "$ROOT/.env" ]] || find "$ROOT" -maxdepth 2 -type f   \( -name '*.pfx' -o -name '*.p12' -o -name '*.key' -o -name '.env.local' \) -print -quit | grep -q .; then
    echo 'STOP: source checkout contains .env/private key file. Remove sensitive files from checkout before mounting into Docker.' >&2
    exit 1
fi
if find "$ROOT/server/ca" -type f ! -name README.md -print -quit 2>/dev/null | grep -q .; then
  echo 'STOP: source checkout contains CA files; move them outside before Docker build' >&2;exit 1
fi
# Fail rather than claiming support for v0.4.2 capabilities in an older binary.
grep -q 'initial_height_px' "$ROOT/client/src/main.rs" || { echo 'STOP: client sources do not include v0.4.2 display controls' >&2;exit 1; }
cat <<LOG
Source: $ROOT
Image: $IMAGE
Building unsigned Windows x64 executable; the Docker image may need to download a Windows SDK.
LOG
# Source is mounted read-write for Cargo output. Run only on a clean, secret-free Git checkout.
docker run --rm \
  -v "$ROOT:/io" -w /io/client \
  -v "$KIT_DIR/.cache/xwin:/root/.cache/xwin" \
  "$IMAGE" cargo xwin build --release --target x86_64-pc-windows-msvc
EXE="$ROOT/client/target/x86_64-pc-windows-msvc/release/vg-status-client.exe"
[[ -s "$EXE" ]] || { echo "Missing build output: $EXE" >&2; exit 1; }
file "$EXE"
file "$EXE" | grep -Eq 'PE32\+|PE32 executable' || { echo 'Output does not look like a Windows PE file' >&2; exit 1; }
cp "$EXE" "$DIST/unsigned/vg-status-client.exe"
sha256sum "$DIST/unsigned/vg-status-client.exe" > "$DIST/unsigned/SHA256SUMS.txt"
printf '\nSUCCESS - unsigned EXE: %s\n' "$DIST/unsigned/vg-status-client.exe"
echo 'NEXT: export config via linux/export-client-config.sh after the Docker build has finished.'
