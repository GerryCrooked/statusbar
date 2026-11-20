#!/bin/sh
# Statusbar: wrap any job (backup, sync, import) so a failure shows a message and the next
# successful run takes it down again.
#
#   STATUSBAR_URL_FILE=/etc/statusbar/webhook-url statusbar-run.sh <key> "<title>" -- <command> [args...]
#
# Example crontab line:
#   30 2 * * *  STATUSBAR_URL_FILE=/etc/statusbar/webhook-url /usr/local/bin/statusbar-run.sh backup-nightly "File server backup" -- /usr/local/bin/backup.sh
#
# Needs statusbar-notify.sh next to it or in PATH. The job's exit code is passed through.
set -u
if [ $# -lt 4 ] || [ "$3" != "--" ]; then
    echo "usage: $0 <key> <title> -- <command> [args...]" >&2
    exit 2
fi
key=$1 title=$2
shift 3
notify=$(dirname "$0")/statusbar-notify.sh
[ -x "$notify" ] || notify=statusbar-notify.sh

output=$("$@" 2>&1)
code=$?
printf '%s\n' "$output"
if [ "$code" -eq 0 ]; then
    STATUSBAR_KEY="$key" STATUSBAR_TITLE="$title" "$notify" ok "$(hostname)" "$key" "" >/dev/null || true
else
    last=$(printf '%s\n' "$output" | tail -n 3)
    STATUSBAR_KEY="$key" STATUSBAR_TITLE="$title" STATUSBAR_SEVERITY="${STATUSBAR_SEVERITY:-warning}" \
        "$notify" fail "$(hostname)" "$key" "Exit code $code. $last" >/dev/null || true
fi
exit "$code"
