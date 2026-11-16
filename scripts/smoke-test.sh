#!/usr/bin/env bash
# End-to-end check against a running server: manual message, monitoring webhook with
# rule, delay, auto-resolve, and what a desktop client would receive.
#   BASE=http://127.0.0.1:8080 ADMIN_TOKEN=... CLIENT_TOKEN=... scripts/smoke-test.sh
set -euo pipefail
BASE="${BASE:-http://127.0.0.1:8080}"
: "${ADMIN_TOKEN:?set ADMIN_TOKEN}" "${CLIENT_TOKEN:?set CLIENT_TOKEN}"
admin() { curl -fsS -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' "$@"; }
client() { curl -fsS -H "Authorization: Bearer $CLIENT_TOKEN" "$BASE/api/client/state?user=test&computer=${1:-PC-TEST-01}&version=9.9.9"; }
field() { sed -n "s/.*\"$1\":\"\\([^\"]*\\)\".*/\\1/p" | head -n1; }
expect() { if printf '%s' "$2" | grep -q -- "$3"; then echo "ok   $1"; else echo "FAIL $1"; echo "     got: $2"; exit 1; fi; }
refuse() { if printf '%s' "$2" | grep -q -- "$3"; then echo "FAIL $1"; echo "     got: $2"; exit 1; else echo "ok   $1"; fi; }

expect "health" "$(curl -fsS "$BASE/health")" "ok"
expect "meta" "$(curl -fsS "$BASE/api/meta")" '"version"'
expect "client token required" "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/client/state")" "401"

id=$(admin -X POST "$BASE/api/admin/incidents" -d '{"title":"Smoke test","message":"manual","severity":"info"}' | field id)
expect "manual message reaches clients" "$(client)" "Smoke test"
admin -X POST "$BASE/api/admin/incidents/$id/resolve" >/dev/null
expect "resolved is delivered as all-clear" "$(client)" '"status":"resolved"'
expect "publisher and resolver are recorded" "$(admin "$BASE/api/admin/incidents")" '"created_by":"api"'
expect "resolver is recorded" "$(admin "$BASE/api/admin/incidents")" '"resolved_by":"api"'
admin -X DELETE "$BASE/api/admin/incidents/$id" >/dev/null

id=$(admin -X POST "$BASE/api/admin/incidents" -d '{"title":"Only accounting","targets":["host:PC-ACC-*"]}' | field id)
expect "host target matches" "$(client PC-ACC-07)" "Only accounting"
refuse "host target excludes others" "$(client PC-HR-01)" "Only accounting"
admin -X DELETE "$BASE/api/admin/incidents/$id" >/dev/null

team=$(admin -X POST "$BASE/api/admin/teams" -d '{"name":"Testers","targets":["host:PC-QA-*"]}' | field id)
id=$(admin -X POST "$BASE/api/admin/incidents" -d '{"title":"Only testers","targets":["team:Testers"]}' | field id)
expect "notification group reaches its members" "$(client PC-QA-03)" "Only testers"
refuse "notification group excludes others" "$(client PC-HR-01)" "Only testers"
expect "unknown notification group is refused" "$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' -X POST "$BASE/api/admin/incidents" -d '{"title":"x","targets":["team:Nobody"]}')" "400"
admin -X DELETE "$BASE/api/admin/incidents/$id" >/dev/null
admin -X DELETE "$BASE/api/admin/teams/$team" >/dev/null

rule=$(admin -X POST "$BASE/api/admin/rules" -d '{"pattern":"smoke-mail*/*","title":"E-mail is disrupted","message":"Affected: {host}"}' | field id)
made=$(admin -X POST "$BASE/api/admin/integrations" -d '{"name":"smoke-test","unmatched":"draft","delay_seconds":0}')
token=$(printf '%s' "$made" | field token); integration=$(printf '%s' "$made" | field id)
hook() { curl -fsS -X POST "$BASE/api/hooks/$token" -H 'Content-Type: application/json' -d "$1"; }

expect "problem with rule is published" "$(hook '{"status":"CRIT","host":"smoke-mail01","service":"SMTP","output":"timeout"}')" "created"
expect "rule text reaches clients" "$(client)" "Affected: smoke-mail01"
expect "repeat is de-duplicated" "$(hook '{"status":"CRIT","host":"smoke-mail01","service":"SMTP"}')" "duplicate"
expect "recovery resolves" "$(hook '{"status":"OK","host":"smoke-mail01","service":"SMTP"}')" "resolved"
expect "monitoring is recorded as publisher" "$(admin "$BASE/api/admin/incidents")" '"created_by":"monitoring:smoke-test"'
expect "problem without rule becomes a draft" "$(hook '{"status":"DOWN","host":"smoke-db9"}')" "draft"
refuse "drafts are not shown" "$(client)" "smoke-db9"
expect "recovery cancels the draft" "$(hook '{"status":"UP","host":"smoke-db9"}')" "cancelled"
expect "alertmanager payload" "$(hook '{"alerts":[{"status":"firing","fingerprint":"smoke1","labels":{"alertname":"SmokeAlert","instance":"smoke-mail02","job":"node"}}]}')" "created"
expect "alertmanager resolve" "$(hook '{"alerts":[{"status":"resolved","fingerprint":"smoke1","labels":{"alertname":"SmokeAlert"}}]}')" "resolved"
expect "unknown token is rejected" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/hooks/sb_0000000000000000000000000000" -d '{}')" "401"

sleep 3
expect "computer list shows desktops with their client version" "$(admin "$BASE/api/admin/computers")" '"version":"9.9.9"'

start=$(date +%s); rev=$(client | sed -n 's/.*"rev":\([0-9]*\).*/\1/p')
curl -fsS -H "Authorization: Bearer $CLIENT_TOKEN" "$BASE/api/client/state?rev=$rev&wait=3" >/dev/null
[ $(( $(date +%s) - start )) -ge 2 ] && echo "ok   long-poll waits when nothing changed" || { echo "FAIL long-poll returned immediately"; exit 1; }

admin -X DELETE "$BASE/api/admin/integrations/$integration" >/dev/null
admin -X DELETE "$BASE/api/admin/rules/$rule" >/dev/null
echo "All checks passed."
