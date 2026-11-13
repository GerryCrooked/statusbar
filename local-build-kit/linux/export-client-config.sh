#!/usr/bin/env bash
set -Eeuo pipefail
umask 077
ENV_PATH="${1:-/opt/docker/vg-statusbar/.env}"
KIT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="${STATUSBAR_DIST:-$KIT_DIR/dist}"
mkdir -p "$DIST/private"
chmod 700 "$DIST/private" "$DIST"
[[ -f "$ENV_PATH" ]] || { echo "Missing .env: $ENV_PATH" >&2;exit 1; }
python3 - "$ENV_PATH" "$DIST/private/client.json" <<'PY'
import json, pathlib, sys
p=pathlib.Path(sys.argv[1]); out=pathlib.Path(sys.argv[2])
values={}
for raw in p.read_text(encoding='utf-8-sig').splitlines():
    s=raw.strip()
    if not s or s.startswith('#') or '=' not in s: continue
    k,v=s.split('=',1)
    k=k.strip();v=v.strip()
    if len(v)>=2 and v[0]==v[-1] and v[0] in ('"',"'"):v=v[1:-1]
    values[k]=v
secret=values.get('CLIENT_TOKEN','')
if not secret or secret.lower().startswith(('change_me','changeme')):
    raise SystemExit('CLIENT_TOKEN missing or placeholder; no config exported')
server_url=values.get('STATUSBAR_SERVER_URL','https://status.vg-ichenhausen.de')
if not server_url.startswith('https://'):
    raise SystemExit('STATUSBAR_SERVER_URL must be https://')
cfg={'server_url':server_url,'client_token':secret,'poll_seconds':int(values.get('CLIENT_POLL_SECONDS','10'))}
if not 5<=cfg['poll_seconds']<=300:raise SystemExit('CLIENT_POLL_SECONDS outside 5..300')
out.write_text(json.dumps(cfg,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
out.chmod(0o600)
PY
printf 'PRIVATE config created: %s (token not printed)\n' "$DIST/private/client.json"
