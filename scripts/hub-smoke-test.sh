#!/usr/bin/env bash
# End-to-end check of the service-provider hub: starts a hub and a customer server
# from the same binary and tests reporting, forwarding, all-clear and the customer's
# own controls.
#   scripts/hub-smoke-test.sh server/target/release/statusbar-server
set -euo pipefail
BIN="${1:?path to the statusbar-server binary}"
work=$(mktemp -d)
cleanup() { kill $(jobs -p) 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT

HUB=http://127.0.0.1:18080
INST=http://127.0.0.1:18081
HA=hub-admin-token-0123456789abcdef0123456
HC=hub-client-token-0123456789abcdef012345
IA=customer-admin-token-0123456789abcdef01
IC=customer-client-token-0123456789abcdef0

up() { for _ in $(seq 1 30); do curl -fsS "$1/health" >/dev/null 2>&1 && return 0; sleep 1; done; echo "FAIL $1 did not start"; exit 1; }
hub()  { curl -fsS -H "Authorization: Bearer $HA" -H 'Content-Type: application/json' "$@"; }
inst() { curl -fsS -H "Authorization: Bearer $IA" -H 'Content-Type: application/json' "$@"; }
desktop()     { curl -fsS -H "Authorization: Bearer $IC" "$INST/api/client/state?user=anna&computer=PC-ACME-01"; }
hub_desktop() { curl -fsS -H "Authorization: Bearer $HC" "$HUB/api/client/state?user=max&computer=PC-MSP-01"; }
field() { sed -n "s/.*\"$1\":\"\\([^\"]*\\)\".*/\\1/p" | head -n1; }
logs() { echo "--- hub log"; tail -n 20 "$work/hub.log"; echo "--- customer log"; tail -n 20 "$work/customer.log"; }
expect() { if printf '%s' "$2" | grep -q -- "$3"; then echo "ok   $1"; else echo "FAIL $1"; echo "     got: $2"; logs; exit 1; fi; }
refuse() { if printf '%s' "$2" | grep -q -- "$3"; then echo "FAIL $1"; echo "     got: $2"; logs; exit 1; else echo "ok   $1"; fi; }
# eventually <seconds> <name> <command> <pattern>
eventually() {
  local out=""
  for _ in $(seq 1 "$1"); do
    out=$(eval "$3" 2>&1 || true)
    if printf '%s' "$out" | grep -q -- "$4"; then echo "ok   $2"; return 0; fi
    sleep 1
  done
  echo "FAIL $2"; echo "     got: $out"; logs; exit 1
}
setting() { # setting <key> <value> on the customer server
  local current; current=$(inst "$INST/api/admin/settings")
  inst -X PUT "$INST/api/admin/settings" -d "$(printf '%s' "$current" | sed "s/\"$1\":\"[a-z]*\"/\"$1\":\"$2\"/")" >/dev/null
}

ADMIN_TOKEN=$HA CLIENT_TOKEN=$HC HUB_ENABLED=yes DATABASE_PATH="$work/hub.sqlite" LISTEN_ADDR=127.0.0.1:18080 \
  "$BIN" >"$work/hub.log" 2>&1 &
up "$HUB"
expect "hub mode is announced" "$(curl -fsS "$HUB/api/meta")" '"hub":true'

made=$(hub -X POST "$HUB/api/admin/customers" -d '{"name":"Acme"}')
token=$(printf '%s' "$made" | field token)
expect "customer created with a token" "$token" "sbh_"
expect "duplicate names are refused" "$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $HA" -H 'Content-Type: application/json' -X POST "$HUB/api/admin/customers" -d '{"name":"acme"}')" "409"

ADMIN_TOKEN=$IA CLIENT_TOKEN=$IC HUB_URL=$HUB HUB_TOKEN=$token DATABASE_PATH="$work/customer.sqlite" LISTEN_ADDR=127.0.0.1:18081 \
  "$BIN" >"$work/customer.log" 2>&1 &
up "$INST"
desktop >/dev/null
eventually 15 "customer server reports in" "hub $HUB/api/admin/customers" '"online":true'
expect "customer reports its version" "$(hub "$HUB/api/admin/customers")" '"version":"0.5.0"'
eventually 15 "customer sees the connection" "inst $INST/api/admin/uplink" '"last_ok":[0-9]'

id=$(hub -X POST "$HUB/api/admin/incidents" -d '{"title":"Provider outage","message":"Mail is down","targets":["customer:Acme"]}' | field id)
eventually 10 "hub message reaches the customer's desktops" desktop "Provider outage"
refuse "hub message stays off the hub's own desktops" "$(hub_desktop)" "Provider outage"
eventually 5 "hub records the delivery to the customer" "hub $HUB/api/admin/incidents" '"customer":"Acme"'
expect "customer sees the service provider as publisher" "$(inst "$INST/api/admin/incidents")" '"created_by":"hub:'

hub -X POST "$HUB/api/admin/incidents" -d '{"title":"Only for others","targets":["customer:Other*"]}' >/dev/null
sleep 3
refuse "messages for other customers stay away" "$(desktop)" "Only for others"

hub -X POST "$HUB/api/admin/incidents/$id/resolve" >/dev/null
eventually 10 "all-clear reaches the customer" desktop '"status":"resolved"'

setting hub_messages approve
hub -X POST "$HUB/api/admin/incidents" -d '{"title":"Needs approval","targets":["customer:*"]}' >/dev/null
eventually 10 "approval mode holds hub messages back" "inst $INST/api/admin/incidents" "Needs approval"
refuse "held messages are not shown" "$(desktop)" "Needs approval"

setting hub_messages off
hub -X POST "$HUB/api/admin/incidents" -d '{"title":"Switched off","targets":["customer:*"]}' >/dev/null
sleep 3
refuse "switched off: hub messages are not accepted" "$(inst "$INST/api/admin/incidents")" "Switched off"

expect "unknown customer token is refused" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$HUB/api/hub/sync" -H 'Authorization: Bearer sbh_00000000000000000000000000000000' -H 'Content-Type: application/json' -d '{}')" "401"
expect "customer targets need hub mode" "$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $IA" -H 'Content-Type: application/json' -X POST "$INST/api/admin/incidents" -d '{"title":"x","targets":["customer:*"]}')" "400"

eventually 40 "customer reports its desktops" "hub $HUB/api/admin/customers" '"clients":1'

customer=$(hub "$HUB/api/admin/customers" | field id)
hub -X DELETE "$HUB/api/admin/customers/$customer" >/dev/null
eventually 40 "removed customer loses the connection" "inst $INST/api/admin/uplink" "rejected the token"
echo "All hub checks passed."
