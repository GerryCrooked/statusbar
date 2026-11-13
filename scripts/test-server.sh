#!/usr/bin/env bash
set -euo pipefail
BASE="${BASE:-http://127.0.0.1:8080}"
: "${ADMIN_TOKEN:?Source .env first: set -a; . ./.env; set +a}"
: "${CLIENT_TOKEN:?Missing CLIENT_TOKEN}"
echo '# Health'
curl -fsS "$BASE/health"; echo
echo '# Create test incident'
reply=$(curl -fsS "$BASE/api/admin/incidents" -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' -d '{"title":"TEST – Statusbar","message":"Nur ein Funktionstest der IT","severity":"warning","target_type":"all"}')
echo "$reply"
id=$(printf '%s' "$reply" | python3 -c 'import json,sys;print(json.load(sys.stdin)["id"])')
echo '# Client JSON'
curl -fsS "$BASE/api/client/state?user=test&computer=test" -H "Authorization: Bearer $CLIENT_TOKEN";echo
echo '# Resolve'
curl -fsS -X POST "$BASE/api/admin/incidents/$id/resolve" -H "Authorization: Bearer $ADMIN_TOKEN"; echo
