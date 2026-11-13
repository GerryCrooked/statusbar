#!/usr/bin/env bash
set -eu
for NAME in ADMIN_TOKEN WEBHOOK_TOKEN CLIENT_TOKEN; do printf '%s=' "$NAME"; openssl rand -hex 32; done
