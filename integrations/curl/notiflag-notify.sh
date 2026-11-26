#!/bin/sh
# Notiflag: generic sender for Nagios, Naemon, Icinga, cron jobs, backup scripts - anything with a shell.
#
#   notiflag-notify.sh <status> <host> [service] [message]
#
#   status   problem:  PROBLEM DOWN CRITICAL WARNING UNKNOWN UNREACHABLE FAIL ERROR 1 ...
#            recovery: OK UP RECOVERY RESOLVED 0 ...
#            anything else is ignored by the server.
#
# Environment (all optional except the URL):
#   NOTIFLAG_URL       https://status.example.com/api/hooks/<token>
#   NOTIFLAG_URL_FILE  file whose first line is that URL. Keeps the token out of
#                       command definitions, logs and `ps`. Used when NOTIFLAG_URL is empty.
#   NOTIFLAG_TYPE      notification type ($NOTIFICATIONTYPE$, $notification.type$). When set,
#                       only PROBLEM and RECOVERY are sent; acknowledgements, downtimes,
#                       flapping and custom notifications are skipped (exit 0).
#   NOTIFLAG_KEY       stable id of the problem. Default (on the server): "<host>/<service>".
#   NOTIFLAG_SEVERITY  error | warning | info (monitoring words like High, Average are mapped).
#   NOTIFLAG_TITLE     title shown when no rule matches. Default: "<service> (<host>)".
#
# Prints the server's answer, e.g. {"ok":true,"events":["created"]}.
# Exit code 0 = sent or skipped; anything else = not delivered (curl's exit code, 22 = HTTP error).
set -eu

if [ $# -lt 2 ]; then
    echo "usage: NOTIFLAG_URL=... $0 <status> <host> [service] [message]" >&2
    exit 2
fi

case "$(printf '%s' "${NOTIFLAG_TYPE:-}" | tr '[:lower:]' '[:upper:]')" in
    "" | PROBLEM | RECOVERY) ;;
    *) echo "notiflag: skipped notification type ${NOTIFLAG_TYPE}"; exit 0 ;;
esac

url="${NOTIFLAG_URL:-}"
if [ -z "$url" ] && [ -n "${NOTIFLAG_URL_FILE:-}" ]; then
    url=$(head -n 1 "$NOTIFLAG_URL_FILE" | tr -d '\r\n\t ')
fi
if [ -z "$url" ]; then
    echo "notiflag: set NOTIFLAG_URL or NOTIFLAG_URL_FILE" >&2
    exit 2
fi

# JSON string: line breaks and tabs become spaces, other control characters are dropped.
esc() { printf '%s' "$1" | tr '\n\r\t' '   ' | tr -d '\000-\037' | sed 's/\\/\\\\/g; s/"/\\"/g'; }
field() { if [ -n "$2" ]; then printf ',"%s":"%s"' "$1" "$(esc "$2")"; fi; }

body="{\"status\":\"$(esc "$1")\",\"host\":\"$(esc "$2")\",\"service\":\"$(esc "${3:-}")\",\"message\":\"$(esc "${4:-}")\""
body="$body$(field key "${NOTIFLAG_KEY:-}")$(field severity "${NOTIFLAG_SEVERITY:-}")$(field title "${NOTIFLAG_TITLE:-}")}"

curl -fsS -m 10 --retry 2 -X POST "$url" -H 'Content-Type: application/json' --data-binary "$body"
echo
