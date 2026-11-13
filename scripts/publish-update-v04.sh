#!/bin/sh
set -eu
SOURCE=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
ROOT=$(mktemp -d)
trap 'rm -rf "$ROOT"' EXIT
REPO="https://github.com/GerryCrooked/statusbar.git"
git clone "$REPO" "$ROOT/repo"
cd "$ROOT/repo"
for f in server/src/main.rs server/static/index.html server/Dockerfile README.md docs/UPDATE-v0.4.md scripts/upgrade-v04.sh scripts/publish-update-v04.sh; do
  mkdir -p "$(dirname "$f")"
  cp "$SOURCE/$f" "$f"
done
mkdir -p server/locales
cp "$SOURCE/server/locales/en.json" "$SOURCE/server/locales/de.json" server/locales/
# Keep the review focused and exclude local/private material.
git add server/src/main.rs server/static/index.html server/Dockerfile server/locales README.md docs/UPDATE-v0.4.md scripts/upgrade-v04.sh scripts/publish-update-v04.sh
git diff --cached --check
if git diff --cached --quiet; then echo 'No changes';exit 0;fi
printf '\nSTAGED FILES:\n'
git diff --cached --name-only
printf '\nReview the staged diff carefully before publishing to a PUBLIC repo.\n'
if [ "${1:-}" = '--check' ]; then
  git diff --cached --stat
  printf '\nPreflight done. Nothing published. Run: bash scripts/publish-update-v04.sh --push\n'
  exit 0
fi
if [ "${1:-}" != '--push' ]; then
  echo 'Usage: bash scripts/publish-update-v04.sh --check|--push' >&2
  exit 2
fi
printf 'Public push? Type PUBLIC: '
read -r consent
[ "$consent" = PUBLIC ] || { echo 'Canceled';exit 1; }
git -c user.name="${GIT_AUTHOR_NAME:-Statusbar Contributor}" -c user.email="${GIT_AUTHOR_EMAIL:-statusbar-contributor@users.noreply.github.com}" commit -m 'feat: add multilingual web UI and translation package management'
git push origin main
