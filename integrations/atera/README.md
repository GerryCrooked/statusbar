# Notiflag for Atera

> **Status: in development.** This integration is being tested against a real Atera account. Settings and
> payload handling may still change; feedback and captured payloads are welcome.

When Atera raises an alert for a customer – a server offline, a disk full, a service stopped – a message
appears on that customer's desktops. When Atera resolves (archives) the alert, the message gets its green
all-clear and disappears.

Atera has outgoing webhooks only on its top plan, so this integration **polls Atera's API**: a small
script runs at the service provider every two minutes, reads the open alerts with one API key and sends
each customer's alerts to that customer's Notiflag.

| File | Purpose |
|---|---|
| [`atera-notiflag.py`](atera-notiflag.py) | The poller (Python 3.8+, standard library only) |

Tested against a stand-in for Atera's API (`/api/v3/alerts` with paging): alerts of two customers and
three severities, archived and removed alerts, Atera unreachable, quotes and backslashes in titles.
**Not yet tested against a real Atera account** – the field names follow Atera's API documentation.

```
cron (every 2 min) ─► atera-notiflag.py ─GET /api/v3/alerts─► Atera
                         │  new alert of a listed customer ──► that customer's Notiflag (problem)
                         └  alert archived or gone ─────────► the same Notiflag (all-clear)
```

## 1. Create the integration in each customer's Notiflag

*Monitoring → Add integration*, name `Atera`, policy **Hold for approval**, delay `0` (the script already
runs every two minutes).
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Atera API key

Atera: *Admin → Data management → API* → copy the **API key**. It can read all customers – keep it on the
service provider's side only, never on a customer server.

## 3. Install the poller

On a Linux host of the service provider:

```bash
install -m 755 atera-notiflag.py /usr/local/bin/atera-notiflag.py
install -d -m 700 /etc/notiflag /var/lib/notiflag
printf '%s\n' '<API key>' > /etc/notiflag/atera-api-key && chmod 600 /etc/notiflag/atera-api-key
cat > /etc/notiflag/atera.json <<'EOF'
{
  "api_key_file": "/etc/notiflag/atera-api-key",
  "state_file": "/var/lib/notiflag/atera-state.json",
  "severities": ["Critical"],
  "customers": {
    "Example GmbH": "https://status.example.com/api/hooks/sb_…",
    "1234": "https://status.other.example/api/hooks/sb_…"
  }
}
EOF
chmod 600 /etc/notiflag/atera.json
atera-notiflag.py --config /etc/notiflag/atera.json --dry-run   # shows what would be sent
```

Crontab of root (or a dedicated user owning the files):

```
*/2 * * * *  /usr/local/bin/atera-notiflag.py --config /etc/notiflag/atera.json
```

Windows: the same with `py atera-notiflag.py --config C:\ProgramData\Notiflag\atera.json` in the task
scheduler, every 2 minutes.

| Setting | Meaning |
|---|---|
| `customers` | Atera customer **name** or **ID** → webhook URL of that customer's Notiflag. Customers not listed are ignored. |
| `severities` | which Atera severities become messages: `Critical`, `Warning`, `Information` (default `Critical`) |
| `state_file` | remembers which alerts were sent, so the all-clear goes to the right Notiflag |
| `api_url` | only for testing (default `https://app.atera.com/api/v3`) |

## 4. How Atera maps to Notiflag

| Atera alert | Notiflag |
|---|---|
| open alert of a listed customer with a selected severity | problem: message opens (sent once) |
| alert archived (resolved) or deleted | recovery: message resolved |
| `AlertID` | key `atera-<id>` |
| `Title` | title (and service, for rules) |
| `DeviceName` | host |
| `AlertMessage` | message text without rule |
| `Severity` *Critical* / *Warning* / *Information* | Outage / Degraded / Notice |

If Atera cannot be reached, the script changes nothing and exits with 1; failed deliveries to a
Notiflag are retried on the next run.

## 5. Example rule

| When host/service matches | Show this title | And this message | Severity |
|---|---|---|---|
| `FS01/*` | `The file server has a problem` | `Network drives may be unavailable. We are working on it.` | As reported |

## Troubleshooting

Run it by hand with `-v`: it prints each sent event and Notiflag's answer.

| Output | Fix |
|---|---|
| `cannot read alerts from Atera: HTTP Error 401` | API key wrong or regenerated. |
| `… not delivered: HTTP Error 401` | Webhook URL of that customer incomplete or integration deleted. |
| Nothing is sent | Customer name differs from Atera (use the ID), or the severity is not in `severities`. `--dry-run` shows the selection. |
| Messages never resolve | The state file is not writable, or a different state file is used per run. |
