#!/usr/bin/env bash
set -euo pipefail
if [ -e .env ]; then echo '.env exists; refusing to overwrite'; exit 1; fi
cp .env.example .env
for key in ADMIN_TOKEN WEBHOOK_TOKEN CLIENT_TOKEN; do
  value=$(openssl rand -hex 32)
  sed -i "s/^${key}=.*/${key}=${value}/" .env
done
chmod 600 .env
echo 'Created .env with three random tokens. Edit AD_* settings before enabling AD_ENABLED=true.'
echo 'To start: docker compose up -d --build'
