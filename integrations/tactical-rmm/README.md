# Statusbar for Tactical RMM

When Tactical RMM raises an alert – an agent overdue (offline), a failed check, a failed task – a message
appears on users' desktops. When the alert resolves, the message gets its green all-clear and disappears.
Tactical RMM's **webhooks** in alert templates do it with a JSON body you paste in.

| File | Purpose |
|---|---|
| [`webhook-failure.json`](webhook-failure.json) | Body of the webhook *Statusbar problem* |
| [`webhook-resolved.json`](webhook-resolved.json) | Body of the webhook *Statusbar resolved* |

**Not tested in Tactical RMM.** Both bodies are tested against Statusbar by
`scripts/test-integrations.sh` with the variables filled in; the variables follow Tactical RMM's
documentation (`{{alert.id}}` is the alert's database ID, the same in failure and resolved actions).

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `Tactical RMM`, policy **Hold for approval**, delay `60`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

For a service provider with several customers: one integration in each customer's Statusbar, and one
alert template per customer (step 2.2).

## 2. Set up Tactical RMM

### 2.1 Two webhooks

*Settings → Global Settings → Web Hooks → Add*, twice:

| Field | Webhook 1 | Webhook 2 |
|---|---|---|
| Name | `Statusbar problem` | `Statusbar resolved` |
| Method | `POST` | `POST` |
| URL Pattern | the webhook URL from step 1 | the same |
| Headers | `{"Content-Type": "application/json"}` | the same |
| Body | [`webhook-failure.json`](webhook-failure.json) | [`webhook-resolved.json`](webhook-resolved.json) |

```json
{
  "key": "tactical-{{alert.id}}",
  "status": "problem",
  "host": "{{agent.hostname}}",
  "service": "{{alert.alert_type}}",
  "severity": "{{alert.severity}}"
}
```

The bodies leave out `{{alert.message}}` on purpose: Tactical RMM inserts variables as they are, and a
check output with a quote or a backslash would break the JSON. Put the words users read into a rule.

### 2.2 Alert template

*Settings → Alerts Manager → Add Alert Template*, e.g. `Statusbar – Example GmbH`:

* **Failure action**: *Send a Web Hook* → `Statusbar problem`
* **Resolved action**: *Send a Web Hook* → `Statusbar resolved`
* **Run actions only on**: tick what users should hear about – typically *Agents* (overdue = offline)
  and *Checks*, with *Error* (and *Warning*) severity.
* Assign the template to the client, site or policy whose servers matter to users
  (*Clients* tree → right-click → *Assign alert template*, or in an automation policy).

## 3. How Tactical RMM maps to Statusbar

| Tactical RMM | Statusbar |
|---|---|
| failure action | problem: message opens |
| resolved action | recovery: message resolved |
| `{{alert.id}}` | key `tactical-<id>` |
| `{{agent.hostname}}` | host |
| `{{alert.alert_type}}` (`availability`, `check`, `task`) | service; title `service (host)` |
| `{{alert.severity}}` `error` / `warning` / `info` | Outage / Degraded / Notice |

## 4. Test

Webhook *Test* in Tactical RMM does **not** fill `{{alert.*}}` variables, so Statusbar receives the
literal key `tactical-{{alert.id}}` – that still proves URL and token. Discard the draft.

End to end: an agent with the template, stop the agent service (`tacticalrmm`) until it is overdue →
message; start it again → all-clear.

## 5. Example rule

| When host/service matches | Show this title | And this message | Severity |
|---|---|---|---|
| `SRV-FS*/availability` | `The file server is not reachable` | `Network drives are not available. We are working on it.` | Outage |

The subject is `<hostname>/<alert type>`.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Statusbar log shows nothing | Template not assigned to the agent's client/site/policy, or *Run actions only on* excludes the alert type. |
| `401` in Tactical's debug log | URL incomplete or integration deleted. |
| `400 JSON body required` | A variable inserted text with quotes – keep the bodies as given. |
| Message stays | No resolved action, or the alert was resolved by hand in Tactical RMM. |
