#!/usr/bin/env bash
set -Eeuo pipefail
REPO="${1:-}"
[[ -n "$REPO" && -d "$REPO/.git" ]] || { echo 'Usage: disable-github-actions.sh /path/to/clean/git/clone' >&2;exit 1; }
cd "$REPO"
[[ -z "$(git status --porcelain)" ]] || { echo 'STOP: git clone has local changes; commit or stash first' >&2;exit 1; }
[[ -f .github/workflows/build.yml ]] || { echo 'No .github/workflows/build.yml to disable';exit 1; }
mkdir -p .github/workflows-disabled
git mv .github/workflows/build.yml .github/workflows-disabled/build.yml
printf 'Workflow moved locally. Review and commit manually ONLY after a successful local signed build:\n  git diff --cached\n  git commit -m "ci: move Windows build to local pipeline"\n  git push origin main\n'
