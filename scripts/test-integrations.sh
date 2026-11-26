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
            ninjaone/test-triggered.json:ninjaone/test-reset.json \
            glpi/test-new.json:glpi/test-solved.json; do
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
# N-able N-central sends XML.
xml() { curl -sS -X POST "$url" -H 'Content-Type: text/xml' --data-binary @"$1"; }
check "n-able/test-failed.xml" "$(xml n-able/test-failed.xml)" created
check "n-able/test-normal.xml" "$(xml n-able/test-normal.xml)" resolved
tactical() { sed -e 's/{{alert.id}}/77/; s/{{agent.hostname}}/TRMM-PC/; s/{{alert.alert_type}}/availability/; s/{{alert.severity}}/error/' \
    "tactical-rmm/$1" | curl -sS -X POST "$url" -H 'Content-Type: application/json' --data-binary @-; }
check "tactical-rmm/webhook-failure.json" "$(tactical webhook-failure.json)" created
check "tactical-rmm/webhook-resolved.json" "$(tactical webhook-resolved.json)" resolved

# Zammad and Jira templates with their placeholders filled in.
zammad() { sed -e 's/#{ticket.number}/99001/; s/#{ticket.group.name}/Users/; s/#{ticket.title}/Itest ticket/g' "zammad/$1" |
    curl -sS -X POST "$url" -H 'Content-Type: application/json' --data-binary @-; }
check "zammad/payload-problem.json" "$(zammad payload-problem.json)" created
check "zammad/payload-resolved.json" "$(zammad payload-resolved.json)" resolved
jira() { sed -e 's/{{issue.key}}/ITEST-1/; s/{{issue.project.name.jsonEncode}}/IT/; s/{{issue.summary.jsonEncode}}/Itest/g; s/{{issue.priority.name.jsonEncode}}/High/' \
    "jira-service-management/$1" | curl -sS -X POST "$url" -H 'Content-Type: application/json' --data-binary @-; }
check "jira-service-management/body-problem.json" "$(jira body-problem.json)" created
check "jira-service-management/body-resolved.json" "$(jira body-resolved.json)" resolved

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

# Outbound tickets through the generic webhook, against scripts/fake-ticket.py. The Statusbar server
# must reach the stand-in: TICKET_HOOK_URL (default http://127.0.0.1:18090/hook). Skipped when a
# ticket system is already configured on the server, so a real one is never touched.
tickets=$(admin "$BASE/api/admin/ticketing")
if [ "$(printf '%s' "$tickets" | jq -r '.config.enabled or .config.secret_set or (.config.url != "")')" = true ]; then
    echo "skip outbound tickets (a ticket system is configured on this server)"
elif ! command -v python3 >/dev/null; then
    echo "skip outbound tickets (no python3)"
else
    port="${TICKET_PORT:-18090}"
    hook="${TICKET_HOOK_URL:-http://127.0.0.1:$port/hook}"
    spool=$(mktemp -d)
    python3 ../scripts/fake-ticket.py --port "$port" --dir "$spool" --token itest-ticket &
    fake=$!
    tickets_off() {
        admin -X PUT "$BASE/api/admin/ticketing" -d '{"enabled":false,"system":"webhook","url":"","clear_secret":true}' >/dev/null || true
        kill "$fake" 2>/dev/null || true
        rm -rf "$spool"
    }
    trap 'tickets_off; cleanup' EXIT
    sleep 1
    admin -X PUT "$BASE/api/admin/ticketing" -d "$(jq -n --arg url "$hook" '{enabled:true, system:"webhook", url:$url,
        secret:"itest-ticket", on_publish:true, on_alarm:false, on_resolve:false, sources:["manual"], resolve_mode:"close"}')" >/dev/null
    seen() { # event, incident id -> the request body, waiting up to 25 s
        for _ in $(seq 50); do
            got=$(jq -c --arg e "$1" --arg id "$2" 'select(.body.event==$e and .body.id==$id)' "$spool/requests.jsonl" 2>/dev/null | head -1)
            [ -n "$got" ] && { printf '%s' "$got"; return; }
            sleep 0.5
        done
    }
    msg=$(admin -X POST "$BASE/api/admin/incidents" -d '{"title":"Ticket \"test\" C:\\temp","message":"line 1\nline 2","severity":"warning"}' | jq -r .id)
    got=$(seen created "$msg")
    if printf '%s' "$got" | jq -e --arg id "$msg" '.auth=="Bearer itest-ticket" and .body.title=="Ticket \"test\" C:\\temp"
        and .body.origin==("statusbar-origin:"+$id) and .body.priority=="normal" and (.body.body|contains("line 2"))' >/dev/null; then
        echo "ok   outbound ticket opened (webhook, values JSON-escaped)"; else echo "FAIL outbound ticket: $got"; fail=1; fi
    for _ in $(seq 20); do
        link=$(admin "$BASE/api/admin/incidents" | jq -r --arg id "$msg" '.incidents[] | select(.id==$id) | .ticket.url // empty')
        [ -n "$link" ] && break; sleep 0.5
    done
    case "$link" in */tickets/*) echo "ok   ticket link shown with the message";; *) echo "FAIL no ticket link ($link)"; fail=1;; esac
    # The ticket coming back through an inbound integration must not open a second message.
    back=$(curl -sS -X POST "$url" -H 'Content-Type: application/json' -d "$(jq -n --arg id "$msg" \
        '{status:"problem", key:"itest-loop", title:"Ticket test", message:("text [statusbar-origin:"+$id+"]")}')")
    check "own ticket through the inbound webhook" "$back" own-ticket
    admin -X POST "$BASE/api/admin/incidents/$msg/resolve" >/dev/null
    got=$(seen resolved "$msg")
    if printf '%s' "$got" | jq -e '.body.ticket_id != "" and (.body.body|startswith("Statusbar: message resolved by"))' >/dev/null; then
        echo "ok   outbound ticket resolved with a note"; else echo "FAIL outbound resolve: $got"; fail=1; fi
    admin -X DELETE "$BASE/api/admin/incidents/$msg" >/dev/null || true
    test=$(admin -X POST "$BASE/api/admin/ticketing/test")
    if printf '%s' "$test" | jq -e '.ok and (.url|test("/tickets/"))' >/dev/null; then echo "ok   test ticket"; else echo "FAIL test ticket: $test"; fail=1; fi
    tickets_off
    trap cleanup EXIT
fi

[ "$fail" -eq 0 ] && echo "All integration checks passed." || { echo "Some integration checks failed."; exit 1; }
