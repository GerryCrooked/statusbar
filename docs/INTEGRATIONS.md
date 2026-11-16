# Monitoring integrations

Statusbar turns alerts from your monitoring into messages on your users' screens –
and takes them down again when the system recovers.

## How it works

1. **Monitoring → Integrations → Add integration.** You get a webhook URL
   `https://<server>/api/hooks/<token>`. The token is shown once; only its hash is stored.
2. Point your monitoring at that URL (recipes below).
3. A **problem** event opens a message, the matching **recovery** resolves it.
   Clients show the green all-clear for the configured time, then the bar disappears.

Three things keep users from being flooded with technical noise:

| Feature | What it does |
|---|---|
| **Rules** | `mail*/*` → "E-mail is disrupted. We are working on it." The first rule whose pattern matches `host/service`, the alert key or its title wins. It sets title, text, severity and recipients, and is published without approval. Placeholders: `{host}` `{service}` `{title}` `{message}`. |
| **Unmatched policy** (per integration) | *Hold for approval* (default): the alert appears under "Waiting for approval" and an admin publishes or discards it with one click. *Publish as received* or *Ignore* are the alternatives. |
| **Delay** (per integration) | Wait N seconds before showing. A blip that recovers in that time never reaches a screen. |

Repeated notifications for the same problem are de-duplicated by key.

## Payload

Generic JSON (`Content-Type: application/json`), one object or an array of objects:

```json
{ "status": "problem", "host": "mail01", "service": "SMTP", "message": "connection timed out",
  "severity": "error", "key": "optional-stable-id", "title": "optional" }
```

* `status` – problem: `problem down critical crit warning warn unknown unreachable firing error fail 1`;
  recovery: `ok up recovery recovered resolved normal 0`. Anything else (acknowledgements, downtimes) is ignored.
* `key` – identifies the problem so the recovery finds it. Defaults to `host/service`.
* `severity` – `error`, `warning` or `info`; common monitoring words (Disaster, High, Average, Warning, Information …) are mapped.

Authentication: token in the URL path, or `POST /api/hooks` with `Authorization: Bearer <token>` or `?token=<token>`.
The response lists what happened per event: `created`, `draft`, `duplicate`, `resolved`, `cancelled`, `ignored`.

Prometheus **Alertmanager**, **Grafana** alerting and **Uptime Kuma** payloads are detected and understood as they are.

## Recipes

| System | How |
|---|---|
| **Zabbix** | Media type *Webhook* with [`integrations/zabbix-webhook.js`](../integrations/zabbix-webhook.js); parameters are listed at the top of the script. Enable it for operations and recovery operations. |
| **Checkmk** | Copy [`integrations/checkmk-statusbar`](../integrations/checkmk-statusbar) to `~/local/share/check_mk/notifications/statusbar`, `chmod +x`, create a notification rule with method *Statusbar* and the webhook URL as first parameter. |
| **Nagios / Naemon / Icinga** | [`integrations/notify.sh`](../integrations/notify.sh) plus the commands in [`integrations/nagios.cfg`](../integrations/nagios.cfg). |
| **Prometheus Alertmanager** | [`integrations/alertmanager.yml`](../integrations/alertmanager.yml) – `webhook_configs` with `send_resolved: true`. |
| **Grafana** | Contact point of type *Webhook*, URL = webhook URL. |
| **Uptime Kuma** | Notification type *Webhook*, Post URL = webhook URL, request body *application/json*. |
| **Anything else** (PRTG, scripts, backup jobs) | `notify.sh`, or one `curl` call with the generic payload. |

## Test from a shell

```bash
URL='https://status.example.com/api/hooks/<token>'
curl -X POST "$URL" -H 'Content-Type: application/json' -d '{"status":"problem","host":"mail01","service":"SMTP"}'
curl -X POST "$URL" -H 'Content-Type: application/json' -d '{"status":"ok","host":"mail01","service":"SMTP"}'
```

`scripts/smoke-test.sh` runs a complete problem → message → recovery cycle against a server.
