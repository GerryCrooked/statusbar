#!/usr/bin/env bash
# Cross-compiles the Windows client on Linux in a container. Output: dist/notiflag-client.exe (unsigned).
# Pin the builder for reproducible releases, e.g. BUILDER=messense/cargo-xwin@sha256:...
# Rebrand folders/window name:  NOTIFLAG_APP_NAME=Acme scripts/build-client.sh
set -euo pipefail
cd "$(dirname "$0")/.."
BUILDER="${BUILDER:-messense/cargo-xwin:latest}"
mkdir -p dist
docker run --rm -v "$PWD/client:/io" -w /io \
  -e NOTIFLAG_APP_NAME="${NOTIFLAG_APP_NAME:-Notiflag}" \
  "$BUILDER" cargo xwin build --release --target x86_64-pc-windows-msvc
cp client/target/x86_64-pc-windows-msvc/release/notiflag-client.exe dist/
sha256sum dist/notiflag-client.exe
