#!/usr/bin/env bash
set -Eeuo pipefail
FROM="${1:-/opt/docker/vg-statusbar}"
TO="${2:-/opt/docker/statusbar-build-source/statusbar}"
[[ -f "$FROM/client/src/main.rs" && -f "$TO/client/src/main.rs" ]] || { echo 'Source files missing' >&2;exit 1; }
[[ -d "$TO/.git" ]] || { echo 'Destination must be separate Git clone' >&2; exit 1; }
[[ "$(realpath "$FROM")" != "$(realpath "$TO")" ]] || { echo 'Source and target are identical' >&2;exit 1; }
grep -q 'initial_height_px' "$FROM/client/src/main.rs" || { echo 'Live source does not contain v0.4.2 features' >&2;exit 1; }
cp "$FROM/client/src/main.rs" "$TO/client/src/main.rs"
cp "$FROM/client/Cargo.toml" "$TO/client/Cargo.toml"
printf 'Copied ONLY two client Rust source files. No .env or certificates copied.\n'
git -C "$TO" status --short -- client/src/main.rs client/Cargo.toml
