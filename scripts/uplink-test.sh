#!/usr/bin/env bash
# End-to-end check of the connection to a service-provider hub, against
# scripts/fake-hub.py (the real hub is a separate product).
#   scripts/uplink-test.sh server/target/release/statusbar-server
set -euo pipefail
BIN="${1:?path to the statusbar-server binary}"
here="$(cd "$(dirname "$0")" && pwd)"
work=$(mktemp -d)
cleanup() { kill $(jobs -p) 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT

HUB=http://127.0.0.1:18080
INST=http://127.0.0.1:18081
HUB_TOKEN=sbh_test-token-0123456789abcdef0123456789
IA=customer-admin-token-0123456789abcdef01
IC=customer-client-token-0123456789abcdef0

up() { for _ in $(seq 1 30); do curl -fsS "$1" >/dev/null 2>&1 && return 0; sleep 1; done; echo "FAIL $1 did not start"; exit 1; }
inst() { curl -fsS -H "Authorization: Bearer $IA" -H 'Content-Type: application/json' "$@"; }
desktop() { curl -fsS -H "Authorization: Bearer $IC" "$INST/api/client/state?user=anna&computer=${1:-PC-ACME-01}&version=9.9.9"; }
report() { cat "$work/report.json" 2>/dev/null || true; }
send() { printf '%s' "$1" > "$work/messages.json.tmp" && mv "$work/messages.json.tmp" "$work/messages.json"; }
logs() { echo "--- server log"; tail -n 20 "$work/server.log"; }
expect() { if printf '%s' "$2" | grep -q -- "$3"; then echo "ok   $1"; else echo "FAIL $1"; echo "     got: $2"; logs; exit 1; fi; }
refuse() { if printf '%s' "$2" | grep -q -- "$3"; then echo "FAIL $1"; echo "     got: $2"; logs; exit 1; else echo "ok   $1"; fi; }
eventually() {
  local out=""
  for _ in $(seq 1 "$1"); do
    out=$(eval "$3" 2>&1 || true)
    if printf '%s' "$out" | grep -q -- "$4"; then echo "ok   $2"; return 0; fi
    sleep 1
  done
  echo "FAIL $2"; echo "     got: $out"; logs; exit 1
}
setting() {
  local current; current=$(inst "$INST/api/admin/settings")
  inst -X PUT "$INST/api/admin/settings" -d "$(printf '%s' "$current" | sed "s/\"$1\":\"[a-z]*\"/\"$1\":\"$2\"/")" >/dev/null
}

send '[]'
python3 -I "$here/fake-hub.py" --port 18080 --token "$HUB_TOKEN" --dir "$work" &
ADMIN_TOKEN=$IA CLIENT_TOKEN=$IC HUB_URL=$HUB HUB_TOKEN=$HUB_TOKEN DATABASE_PATH="$work/server.sqlite" LISTEN_ADDR=127.0.0.1:18081 \
  "$BIN" >"$work/server.log" 2>&1 &
up "$INST/health"

expect "connection is announced" "$(curl -fsS "$INST/api/meta")" '"uplink":true'
eventually 15 "server reports to the hub" report '"version":"0.5.0"'
eventually 10 "server sees the connection" "inst $INST/api/admin/uplink" '"last_ok":[0-9]'

desktop >/dev/null
eventually 5 "computer list shows the desktop with its client version" "inst $INST/api/admin/computers" '"version":"9.9.9"'
send '[]'   # new revision: makes the server report again right away
eventually 10 "hub receives the computer list" report '"computer":"pc-acme-01"'
refuse "hub does not receive user names" "$(report)" 'anna'

send '[{"id":"m1","title":"Provider outage","message":"Mail is down","severity":"error","status":"active","teams":[]}]'
eventually 10 "hub message reaches the desktops" desktop "Provider outage"
expect "service provider is recorded as publisher" "$(inst "$INST/api/admin/incidents")" '"created_by":"hub:Test hub"'
eventually 10 "active message is reported back" report '"title":"Provider outage"'
expect "hub learns the message is shown" "$(report)" '"id":"m1","state":"shown"'

send '[{"id":"m1","title":"Provider outage","message":"Mail is down","severity":"error","status":"resolved","teams":[]}]'
eventually 10 "all-clear reaches the desktops" desktop '"status":"resolved"'

inst -X POST "$INST/api/admin/teams" -d '{"name":"Finance","targets":["host:PC-FIN-*"]}' >/dev/null
send '[]'
eventually 10 "notification groups are reported" report '"teams":\["Finance"\]'
send '[{"id":"m2","title":"For finance only","message":"","severity":"warning","status":"active","teams":["Finance"]}]'
eventually 10 "message for a group reaches that group" "desktop PC-FIN-07" "For finance only"
refuse "other desktops do not get it" "$(desktop PC-ACME-01)" "For finance only"

setting hub_messages approve
send '[{"id":"m3","title":"Needs approval","message":"","severity":"info","status":"active","teams":[]}]'
eventually 10 "approval mode holds hub messages back" "inst $INST/api/admin/incidents" "Needs approval"
refuse "held messages are not shown" "$(desktop)" "Needs approval"
touch "$work/messages.json"   # new revision: report again
eventually 10 "hub learns the message waits for approval" report '"id":"m3","state":"pending"'
expect "hub learns the approval setting" "$(report)" '"accepts":"approve"'
held=$(inst "$INST/api/admin/incidents" | jq -r '.incidents[]|select(.title=="Needs approval")|.id')
inst -X DELETE "$INST/api/admin/incidents/$held" >/dev/null
# No new revision here: the server must drop its waiting long-poll (25 s) and report at once.
eventually 10 "hub learns the message was declined right away" report '"id":"m3","state":"discarded","by":"api"'
sleep 2
expect "declined message does not come back" "$(inst "$INST/api/admin/incidents" | jq -r '[.incidents[]|select(.title=="Needs approval")|.status]|join(",")')" '^discarded$'
held_end() { inst "$INST/api/admin/incidents" | jq -r '.incidents[]|select(.title=="Needs approval")|.ends_at'; }
send '[{"id":"m3","title":"Needs approval","message":"","severity":"info","status":"resolved","teams":[]}]'
eventually 10 "provider's all-clear is noted on the declined message" held_end '^[0-9]'

setting hub_messages off
send '[{"id":"m4","title":"Switched off","message":"","severity":"info","status":"active","teams":[]}]'
sleep 3
refuse "switched off: hub messages are not accepted" "$(inst "$INST/api/admin/incidents")" "Switched off"

kill %1
python3 -I "$here/fake-hub.py" --port 18080 --token other-token-0000000000000000000 --dir "$work" &
eventually 40 "rejected token is shown" "inst $INST/api/admin/uplink" "rejected the token"
echo "All connection checks passed."
