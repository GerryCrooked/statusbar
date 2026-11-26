#!/bin/sh
# Notiflag: wrap any job (backup, sync, import) so a failure shows a message and the next
# successful run takes it down again.
#
#   NOTIFLAG_URL_FILE=/etc/notiflag/webhook-url notiflag-run.sh <key> "<title>" -- <command> [args...]
#
# Example crontab line:
#   30 2 * * *  NOTIFLAG_URL_FILE=/etc/notiflag/webhook-url /usr/local/bin/notiflag-run.sh backup-nightly "File server backup" -- /usr/local/bin/backup.sh
#
# Needs notiflag-notify.sh next to it or in PATH. The job's exit code is passed through.
set -u
if [ $# -lt 4 ] || [ "$3" != "--" ]; then
    echo "usage: $0 <key> <title> -- <command> [args...]" >&2
    exit 2
fi
key=$1 title=$2
shift 3
notify=$(dirname "$0")/notiflag-notify.sh
[ -x "$notify" ] || notify=notiflag-notify.sh

output=$("$@" 2>&1)
code=$?
printf '%s\n' "$output"
if [ "$code" -eq 0 ]; then
    NOTIFLAG_KEY="$key" NOTIFLAG_TITLE="$title" "$notify" ok "$(hostname)" "$key" "" >/dev/null || true
else
    last=$(printf '%s\n' "$output" | tail -n 3)
    NOTIFLAG_KEY="$key" NOTIFLAG_TITLE="$title" NOTIFLAG_SEVERITY="${NOTIFLAG_SEVERITY:-warning}" \
        "$notify" fail "$(hostname)" "$key" "Exit code $code. $last" >/dev/null || true
fi
exit "$code"
