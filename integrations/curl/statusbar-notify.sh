#!/bin/sh
# Statusbar: generic sender for Nagios, Naemon, Icinga, cron jobs, backup scripts - anything with a shell.
#
#   statusbar-notify.sh <status> <host> [service] [message]
#
#   status   problem:  PROBLEM DOWN CRITICAL WARNING UNKNOWN UNREACHABLE FAIL ERROR 1 ...
#            recovery: OK UP RECOVERY RESOLVED 0 ...
#            anything else is ignored by the server.
#
# Environment (all optional except the URL):
#   STATUSBAR_URL       https://status.example.com/api/hooks/<token>
#   STATUSBAR_URL_FILE  file whose first line is that URL. Keeps the token out of
#                       command definitions, logs and `ps`. Used when STATUSBAR_URL is empty.
#   STATUSBAR_TYPE      notification type ($NOTIFICATIONTYPE$, $notification.type$). When set,
#                       only PROBLEM and RECOVERY are sent; acknowledgements, downtimes,
#                       flapping and custom notifications are skipped (exit 0).
#   STATUSBAR_KEY       stable id of the problem. Default (on the server): "<host>/<service>".
#   STATUSBAR_SEVERITY  error | warning | info (monitoring words like High, Average are mapped).
#   STATUSBAR_TITLE     title shown when no rule matches. Default: "<service> (<host>)".
#
# Prints the server's answer, e.g. {"ok":true,"events":["created"]}.
# Exit code 0 = sent or skipped; anything else = not delivered (curl's exit code, 22 = HTTP error).
set -eu

if [ $# -lt 2 ]; then
    echo "usage: STATUSBAR_URL=... $0 <status> <host> [service] [message]" >&2
    exit 2
fi

case "$(printf '%s' "${STATUSBAR_TYPE:-}" | tr '[:lower:]' '[:upper:]')" in
    "" | PROBLEM | RECOVERY) ;;
    *) echo "statusbar: skipped notification type ${STATUSBAR_TYPE}"; exit 0 ;;
esac

url="${STATUSBAR_URL:-}"
if [ -z "$url" ] && [ -n "${STATUSBAR_URL_FILE:-}" ]; then
    url=$(head -n 1 "$STATUSBAR_URL_FILE" | tr -d '\r\n\t ')
fi
if [ -z "$url" ]; then
    echo "statusbar: set STATUSBAR_URL or STATUSBAR_URL_FILE" >&2
    exit 2
fi

# JSON string: line breaks and tabs become spaces, other control characters are dropped.
esc() { printf '%s' "$1" | tr '\n\r\t' '   ' | tr -d '\000-\037' | sed 's/\\/\\\\/g; s/"/\\"/g'; }
field() { if [ -n "$2" ]; then printf ',"%s":"%s"' "$1" "$(esc "$2")"; fi; }

body="{\"status\":\"$(esc "$1")\",\"host\":\"$(esc "$2")\",\"service\":\"$(esc "${3:-}")\",\"message\":\"$(esc "${4:-}")\""
body="$body$(field key "${STATUSBAR_KEY:-}")$(field severity "${STATUSBAR_SEVERITY:-}")$(field title "${STATUSBAR_TITLE:-}")}"

curl -fsS -m 10 --retry 2 -X POST "$url" -H 'Content-Type: application/json' --data-binary "$body"
echo
