# Notiflag for GLPI

When a GLPI ticket for a known outage is opened – say "VPN not working" in the category *Outage for all
users* – a message appears on every desktop, so users see that IT already knows. When the ticket is
solved or closed, the message gets its green all-clear and disappears.

GLPI's built-in **webhooks** send their standard JSON; Notiflag reads it as it is. You do **not** need a
custom payload (GLPI's custom payloads do not escape backslashes, so ticket titles like `C:\temp` would
break them).

| File | Purpose |
|---|---|
| [`test-new.json`](test-new.json), [`test-solved.json`](test-solved.json) | Real GLPI payloads (ticket created, solved) for testing with `curl` |

Tested with GLPI 12.0: webhooks for *new* and *update*, a ticket created and solved through GLPI, title
with quotes and backslashes, HTML in the description. Webhooks exist since GLPI 10.0; the filter tab and
the private-network setting in step 2.1 are from newer versions – older ones simply lack them.

## 1. Create the integration in Notiflag

*Monitoring → Add integration*, name `GLPI`, policy **Hold for approval**, delay `0`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Set up GLPI

### 2.1 Allow requests to your internal network (GLPI 11/12)

GLPI blocks webhooks to private and local addresses by default; the webhook log then says
`Host "10.0.0.5" is blocked for "https://status.example.local/…"`. If Notiflag runs on your internal
network, allow webhooks in **`config/local_define.php`** of the GLPI installation (e.g.
`/var/www/glpi/config/local_define.php`, not the data directory):

```php
<?php
// Allow webhooks to Notiflag on the internal network. Agent and Auth are GLPI's defaults.
define('GLPI_SERVERSIDE_URL_ALLOWED_PRIVATE_NETWORKS_CONTEXTS', ['Agent', 'Auth', 'Webhook']);
```

In the official Docker image mount that file to `/var/www/glpi/config/local_define.php`. Then
`php bin/console cache:clear`.

### 2.2 A category or criterion for "users should see this"

Pick what marks a ticket for Notiflag, e.g. a ticket category `Outage for all users`
(*Setup → Dropdowns → ITIL categories*). Agents choose it for known outages.

### 2.3 Two webhooks

*Setup → Webhooks → Add*, twice (only *Event* differs):

| Field | Value |
|---|---|
| Name | `Notiflag new` / `Notiflag update` |
| Active | yes |
| Itemtype | **Ticket** |
| Event | **New** / **Update** |
| URL | the webhook URL from step 1 |
| HTTP method | `POST` |
| Use default payload | **yes** |
| Secret | leave the generated value, Notiflag ignores the signature |

On each webhook, tab **Filter** (*Webhook target filter*): *ITIL category* **is** `Outage for all
users` – otherwise every ticket becomes a message.

GLPI sends webhooks from its cron task `queuedwebhook` (every minute). Make sure GLPI's cron runs
(*Setup → Automatic actions*, mode *CLI*).

## 3. How GLPI maps to Notiflag

| GLPI ticket | Notiflag |
|---|---|
| status *New*, *Processing (assigned)*, *Processing (planned)*, *Pending* | problem: message opens |
| status *Solved* or *Closed*, ticket in the trash, event *delete* | recovery: message resolved |
| ticket ID | key `glpi-<id>` |
| title | title |
| description (HTML removed) | message text |
| priority *Major*, *Very high* / *High*, *Medium* / *Low*, *Very low* | Outage / Degraded / Notice |
| entity / category (full names) | host / service |

Further updates of the open ticket are answered with `duplicate`. A reopened ticket opens a new message.
If a ticket stops matching the filter (category changed), GLPI sends nothing more – resolve the message
in Notiflag.

## 4. Test

Create a ticket in the category, then solve it. *Setup → Webhooks →* the webhook *→ Queries* shows each
call with HTTP status (`200`). On the Notiflag side the integration's counter goes up.

Without GLPI:

```bash
URL='https://status.example.com/api/hooks/<token>'
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-new.json
# {"events":["draft"],"ok":true}
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-solved.json
# {"events":["cancelled"],"ok":true}
```

## 5. Example rule

| When host/service matches | Show this title | And this message | Severity |
|---|---|---|---|
| `*/Outage for all users` | `{title}` | `IT is working on it. No need to open a ticket.` | As reported |

The subject is `<entity>/<category>`, e.g. `Root entity/Outage for all users`.

## Troubleshooting

GLPI's log: `files/_log/webhook.log` (Docker: `/var/glpi/logs/webhook.log`).

| Symptom | Fix |
|---|---|
| `Host "…" is blocked for "…"` | Step 2.1. |
| Queries show `401` | URL incomplete or integration deleted. |
| Nothing is sent | Webhook inactive, cron `queuedwebhook` not running, or the ticket does not match the filter. |
| Every ticket becomes a message | No filter on the webhooks. |
| `{"events":[],"ok":true}` | *Use default payload* is off. |
