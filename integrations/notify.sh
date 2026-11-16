#!/bin/sh
# Generic sender for Nagios, Icinga, Naemon, cron jobs, backup scripts - anything with a shell.
#   STATUSBAR_URL=https://status.example.com/api/hooks/<token> notify.sh <status> <host> [service] [message]
# status: PROBLEM|DOWN|CRITICAL|WARNING|UNKNOWN ... or OK|UP|RECOVERY
set -eu
: "${STATUSBAR_URL:?set STATUSBAR_URL}"
esc() { printf '%s' "$1" | tr '\n\r\t' '   ' | sed 's/\\/\\\\/g; s/"/\\"/g'; }
curl -fsS -m 10 -X POST "$STATUSBAR_URL" -H 'Content-Type: application/json' \
  -d "{\"status\":\"$(esc "$1")\",\"host\":\"$(esc "$2")\",\"service\":\"$(esc "${3:-}")\",\"message\":\"$(esc "${4:-}")\"}"
