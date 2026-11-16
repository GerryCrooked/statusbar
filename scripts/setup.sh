#!/usr/bin/env bash
# Creates .env with fresh random tokens. Never overwrites an existing .env.
set -euo pipefail
cd "$(dirname "$0")/.."
if [ -e .env ]; then echo ".env already exists - nothing changed."; exit 1; fi
cp .env.example .env
for key in ADMIN_TOKEN CLIENT_TOKEN; do
  sed -i "s/^${key}=.*/${key}=$(openssl rand -hex 32)/" .env
done
chmod 600 .env
echo "Created .env. Start with:  docker compose up -d --build"
echo "Sign in at http://127.0.0.1:8080 with the ADMIN_TOKEN from .env (put HTTPS in front for real use)."
