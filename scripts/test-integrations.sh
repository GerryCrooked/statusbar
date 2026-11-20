#!/usr/bin/env bash
# Replays every sample payload and runs every bundled sender from integrations/ against a running
# Statusbar server: each problem must open a message and its recovery must resolve it.
# Creates a temporary integration ("integration-test", publish, no delay) and removes it, together
# with the messages it created, at the end.
#   BASE=http://127.0.0.1:8080 ADMIN_TOKEN=... scripts/test-integrations.sh
# Needs curl and jq; python3 for the Checkmk plugin, pwsh for the PowerShell script (skipped if missing).
set -euo pipefail
BASE="${BASE:-http://127.0.0.1:8080}"
: "${ADMIN_TOKEN:?set ADMIN_TOKEN}"
cd "$(dirname "$0")/../integrations"
admin() { curl -fsS -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' "$@"; }
fail=0
check() { # name, got, expected outcome
    if printf '%s' "$2" | jq -e --arg want "$3" '.events == [$want]' >/dev/null 2>&1; then echo "ok   $1"; else echo "FAIL $1 (want $3, got: $2)"; fail=1; fi
}

made=$(admin -X POST "$BASE/api/admin/integrations" -d '{"name":"integration-test","unmatched":"publish","delay_seconds":0}')
id=$(printf '%s' "$made" | jq -r .integration.id)
token=$(printf '%s' "$made" | jq -r .token)
url="$BASE/api/hooks/$token"
cleanup() {
    for m in $(admin "$BASE/api/admin/incidents" | jq -r '.incidents[] | select(.created_by=="monitoring:integration-test") | .id'); do
        admin -X DELETE "$BASE/api/admin/incidents/$m" >/dev/null || true
    done
    admin -X DELETE "$BASE/api/admin/integrations/$id" >/dev/null || true
}
trap cleanup EXIT
post() { curl -sS -X POST "$url" -H 'Content-Type: application/json' --data-binary @"$1"; }

for pair in curl/test-problem.json:curl/test-recovery.json \
            alertmanager/test-firing.json:alertmanager/test-resolved.json \
            grafana/test-firing.json:grafana/test-resolved.json \
            uptime-kuma/test-down.json:uptime-kuma/test-up.json \
            azure-monitor/test-fired.json:azure-monitor/test-resolved.json \
            azure-monitor/test-servicehealth-active.json:azure-monitor/test-servicehealth-resolved.json \
            ninjaone/test-triggered.json:ninjaone/test-reset.json; do
    problem=${pair%%:*} recovery=${pair##*:}
    check "$problem" "$(post "$problem")" created
    check "$problem (repeat)" "$(post "$problem")" duplicate
    check "$recovery" "$(post "$recovery")" resolved
done
got=$(post uptime-kuma/test-notification.json)
if [ "$(printf '%s' "$got" | jq -c .events)" = "[]" ]; then echo "ok   uptime-kuma/test-notification.json (no event)"; else echo "FAIL test notification: $got"; fail=1; fi

# PRTG HTTP action payload with placeholders filled in, sent as PRTG does (form content type).
prtg() { sed -e "s/%sensorid/9999/; s/%status/$1/; s/%device/prtg-test/; s/%name/HTTP action/" prtg/http-action-payload.txt |
    curl -sS -X POST "$url" -H 'Content-Type: application/x-www-form-urlencoded' --data-binary @-; }
check "prtg/http-action-payload.txt Down" "$(prtg Down)" created
check "prtg/http-action-payload.txt Up" "$(prtg Up)" resolved

# LibreNMS API transport body and Datto RMM payloads with their variables filled in.
lnms() { sed -e "s/{{ \$alert_id }}/9998/; s/{{ \$state }}/$1/" -e 's/{{ \$sysName|json_encode }}/"lnms-test"/' \
    -e 's/{{ \$name|json_encode }}/"Devices up\/down"/; s/{{ \$title|json_encode }}/"Alert"/; s/{{ \$severity }}/critical/' librenms/api-body.json |
    curl -sS -X POST "$url" -H 'Content-Type: application/json' --data-binary @-; }
check "librenms/api-body.json alert" "$(lnms 1)" created
check "librenms/api-body.json recovered" "$(lnms 0)" resolved
datto() { sed -e 's/\[alert_uid\]/itest-9997/; s/\[device_hostname\]/DATTO-TEST/' "datto-rmm/$1" |
    curl -sS -X POST "$url" -H 'Content-Type: application/json' --data-binary @-; }
check "datto-rmm/alert-raised.json" "$(datto alert-raised.json)" created
check "datto-rmm/alert-resolved.json" "$(datto alert-resolved.json)" resolved

export STATUSBAR_URL="$url"
check "curl/statusbar-notify.sh CRITICAL" "$(sh curl/statusbar-notify.sh CRITICAL itest 'quote " back\ slash' "line 1
line 2	tab")" created
check "curl/statusbar-notify.sh OK" "$(sh curl/statusbar-notify.sh OK itest 'quote " back\ slash')" resolved
if [ "$(STATUSBAR_TYPE=ACKNOWLEDGEMENT sh curl/statusbar-notify.sh CRITICAL itest x | cut -c1-18)" = "statusbar: skipped" ]; then
    echo "ok   curl/statusbar-notify.sh skips acknowledgements"; else echo "FAIL acknowledgement was sent"; fail=1; fi
job() { admin "$BASE/api/admin/incidents" | jq -r '[.incidents[] | select(.created_by=="monitoring:integration-test" and .title=="Integration test job")][0].status'; }
code=0; sh curl/statusbar-run.sh itest-job "Integration test job" -- sh -c 'echo broken; exit 3' >/dev/null || code=$?
if [ "$code" = 3 ] && [ "$(job)" = active ]; then echo "ok   curl/statusbar-run.sh failed job opens a message"; else echo "FAIL statusbar-run.sh failure (exit $code, $(job))"; fail=1; fi
sh curl/statusbar-run.sh itest-job "Integration test job" -- true >/dev/null
if [ "$(job)" = resolved ]; then echo "ok   curl/statusbar-run.sh next good run resolves it"; else echo "FAIL statusbar-run.sh success ($(job))"; fail=1; fi
unset STATUSBAR_URL

if command -v python3 >/dev/null; then
    cmk() { env NOTIFY_PARAMETER_1="$url" NOTIFY_WHAT=SERVICE NOTIFY_HOSTNAME=itest NOTIFY_SERVICEDESC=Checkmk "$@" python3 checkmk/statusbar | sed 's/^Statusbar: //'; }
    check "checkmk/statusbar PROBLEM" "$(cmk NOTIFY_NOTIFICATIONTYPE=PROBLEM NOTIFY_SERVICESTATE=CRITICAL NOTIFY_SERVICEOUTPUT=test)" created
    check "checkmk/statusbar RECOVERY" "$(cmk NOTIFY_NOTIFICATIONTYPE=RECOVERY NOTIFY_SERVICESTATE=OK)" resolved
else
    echo "skip checkmk/statusbar (no python3)"
fi

if command -v pwsh >/dev/null; then
    psh() { pwsh -NoProfile -File prtg/statusbar.ps1 -Url "$url" -Device itest -Sensor PowerShell -SensorId itest "$@" | sed 's/^Statusbar: //'; }
    check "prtg/statusbar.ps1 Down" "$(psh -Status Down -Message "can't connect")" created
    check "prtg/statusbar.ps1 Up" "$(psh -Status Up)" resolved
else
    echo "skip prtg/statusbar.ps1 (no pwsh)"
fi

[ "$fail" -eq 0 ] && echo "All integration checks passed." || { echo "Some integration checks failed."; exit 1; }
