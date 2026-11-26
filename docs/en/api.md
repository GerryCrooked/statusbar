---
id: api
title: API
description: HTTP API of the Notiflag server – messages from scripts, monitoring webhooks, settings, groups and status.
lang: en
slug: api
order: 15
---

# API

Everything the web UI does is available over HTTP. All calls speak JSON.

## Authentication

| Area | Authentication |
|---|---|
| `/api/admin/…` | `Authorization: Bearer <ADMIN_TOKEN>` – actions appear in the history as *API* –, or the web UI's session cookie |
| `/api/hooks/…` | An integration's token (see [Connect your monitoring](monitoring.md#generic-format)) |
| `/api/client/…`, `/api/sounds/…` | `Authorization: Bearer <CLIENT_TOKEN>` |
| `/health`, `/api/meta` | none |

Errors answer with an HTTP status and a short text, e.g. `400 Title required (max. 120 characters)`.

## Messages

| Call | Effect |
|---|---|
| `GET /api/admin/incidents` | The latest 200 messages |
| `POST /api/admin/incidents` | Publish a message |
| `POST /api/admin/incidents/{id}/resolve` | Resolve |
| `POST /api/admin/incidents/{id}/publish` | Approve a waiting message |
| `DELETE /api/admin/incidents/{id}` | Delete, or discard a waiting message |

Create a message:

```bash
curl -X POST https://status.example.com/api/admin/incidents \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' \
  -d '{"title":"Maintenance tonight","message":"ERP offline 8-9 pm","severity":"info",
       "targets":["host:PC-ACC-*","team:Accounting"],"starts_at":1795190400,"ends_at":1795194000}'
```

| Field | Required | Meaning |
|---|---|---|
| `title` | yes | At most 120 characters |
| `message` | | At most 1500 characters |
| `severity` | | `error` (default), `warning`, `info` |
| `targets` | | Empty = everyone. Otherwise up to 50 entries `kind:value`: `host:<pattern>`, `team:<notification group>`, with Active Directory `group:<DN>`, `ou:<DN>`, `user:<sign-in name>` |
| `starts_at` | | Unix seconds; show only from then |
| `ends_at` | | Unix seconds; resolve automatically then |

A message in the answer and in the list:

```json
{ "id": "6f1c…", "title": "Maintenance tonight", "message": "…", "severity": "info",
  "targets": ["host:PC-ACC-*"], "status": "active", "source": "manual", "ext_key": null,
  "created_at": 1795170000, "starts_at": 1795190400, "ends_at": 1795194000, "resolved_at": null,
  "created_by": "api", "approved_by": null, "resolved_by": null }
```

`status`: `draft` (waiting for approval), `active`, `resolved`, `discarded`. `severity` can also be `alarm` for alarms.
`created_by` / `resolved_by`: user name, `admin`, `api`, `monitoring:<integration>`, `hub:<hub>`,
`alarm:<user>@<computer>` or `schedule`.

## Monitoring

| Call | Effect |
|---|---|
| `POST /api/hooks/{token}` (or `/api/hooks` with bearer token or `?token=`) | Report a problem or recovery – format in [Connect your monitoring](monitoring.md#generic-format) |
| `GET /api/admin/integrations` | Integrations and rules |
| `POST /api/admin/integrations` | `{"name","unmatched":"draft\|publish\|ignore","delay_seconds","targets"}` – the answer contains `token` once |
| `DELETE /api/admin/integrations/{id}` | Delete an integration |
| `POST /api/admin/rules` | `{"pattern","title","message","severity","targets"}` (`severity` empty = as reported) |
| `DELETE /api/admin/rules/{id}` | Delete a rule |
| `GET /api/admin/m365` | State of Microsoft 365 service health |

## Settings, groups, sounds

| Call | Effect |
|---|---|
| `GET /api/admin/settings`, `PUT /api/admin/settings` | All settings as one object (read, change, write back whole) |
| `PUT /api/admin/logo` (raw image bytes), `DELETE /api/admin/logo` | Logo |
| `GET /api/admin/sounds`, `PUT /api/admin/sounds/{kind}` (WAV), `DELETE /api/admin/sounds/{kind}` | Own sounds; `{kind}` = `alarm`, `error`, `warning`, `info`, `resolved` |
| `GET /api/admin/teams`, `POST /api/admin/teams` `{"name","targets"}`, `PUT/DELETE /api/admin/teams/{id}` | Notification groups |
| `GET /api/admin/targets` | Recipient suggestions: computers, groups, OUs, users, notification groups |

## Computers, deployment, status

| Call | Effect |
|---|---|
| `GET /api/admin/computers` | Computers with the client: name, last user, client version, last contact |
| `GET /api/admin/client.msi` | The installer |
| `GET /api/admin/client-package.zip` | ZIP with installer, `install.cmd`, `uninstall.cmd` for this server |
| `GET /api/admin/license` | License state |
| `GET /api/admin/uplink` | Hub connection state |
| `GET /health` | `ok` when the server runs |
| `GET /api/meta` | Version, name, mode (`standalone` or `directory`) |

## For clients

The Windows client uses these calls; they are listed for completeness.

| Call | Effect |
|---|---|
| `GET /api/client/state?user=&computer=&version=&rev=&wait=` | What a desktop should show; long-poll up to `wait` seconds (at most 55) |
| `POST /api/client/alarm` `{"user","computer"}` | Silent alarm (only when switched on) |
| `GET /api/sounds/{kind}` | An uploaded sound |
