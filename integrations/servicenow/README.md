# Statusbar for ServiceNow

When a priority 1 incident is opened in ServiceNow – or whatever marks an outage for users in your
instance – a message appears on every desktop, so users see that IT already knows. When the incident is
resolved, closed or cancelled, the message gets its green all-clear and disappears.

A **business rule** on the incident table does it with ServiceNow's outbound REST API.

| File | Purpose |
|---|---|
| [`statusbar-business-rule.js`](statusbar-business-rule.js) | Script of the business rule |

**Not tested in a ServiceNow instance.** The request is the generic payload tested by
`scripts/test-integrations.sh`; the script uses only long-standing server APIs (`GlideRecord`,
`sn_ws.RESTMessageV2`, `gs.getProperty`). Try it in a sub-production instance first.

Statusbar must be reachable from your instance: over the internet with a publicly trusted certificate,
or through a **MID Server** on your network (see step 3).

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `ServiceNow`, policy **Hold for approval**, delay `0`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. System property for the URL

*All → System Properties → All Properties* (`sys_properties.list`) *→ New*:

| Field | Value |
|---|---|
| Name | `statusbar.webhook_url` |
| Type | `string` |
| Value | the webhook URL from step 1 |
| Read roles | `admin` |
| Write roles | `admin` |

## 3. Business rule

*All → System Definition → Business Rules → New*:

| Field | Value |
|---|---|
| Name | `Statusbar` |
| Table | `Incident [incident]` |
| Active | yes |
| Advanced | yes |
| When to run | **after**, **Insert** and **Update**, no filter conditions |
| Advanced → Script | content of [`statusbar-business-rule.js`](statusbar-business-rule.js) |

The script decides itself:

* **Which incidents**: `function flagged()` – by default *Priority 1 – Critical*. Change it to a major
  incident (`current.getValue('major_incident_state') == 'accepted'`), a category, an assignment group
  or a field of your own.
* **Problem**: flagged and not *Resolved*/*Closed*/*Canceled*.
* **All-clear**: resolved, closed or cancelled – or no longer flagged (priority lowered).
* Comments and work notes alone send nothing.

Statusbar on your internal network: remove `//` before `request.setMIDServer('…')` and enter your MID
Server's name.

## 4. How ServiceNow maps to Statusbar

| ServiceNow | Statusbar |
|---|---|
| incident flagged and open | problem: message opens |
| state *Resolved*, *Closed*, *Canceled*, or no longer flagged | recovery: message resolved |
| number | key `snow-INC0012345` |
| short description | title |
| configuration item (else assignment group) | host |
| business service (else category) | service |
| impact *1 – High* / other | Outage / Degraded |

## 5. Test

Create an incident with impact and urgency *1 – High* (priority 1). Statusbar shows a draft. Resolve it:
the draft is deleted (`cancelled`) or the message resolved.

Requests and answers: *System Logs → Outbound HTTP Requests* (`sys_outbound_http_log`). Script warnings:
*System Logs → Warnings*.

## 6. Example rule

| When host/service matches | Show this title | And this message | Severity |
|---|---|---|---|
| `*/E-Mail*` | `E-mail is disrupted` | `IT is working on it. No need to open a ticket.` | As reported |

The subject is `<CI or assignment group>/<business service or category>`.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Warning `system property statusbar.webhook_url is not set` | Step 2. |
| Outbound log: `401` | URL incomplete or integration deleted. |
| Outbound log: connection / certificate error | Statusbar not reachable from the instance; use a MID Server, or a publicly trusted certificate. |
| Nothing in the outbound log | Business rule inactive, not *after* / *Insert* + *Update*, or `flagged()` never true. |
| Message stays | The incident was edited by a process that does not run business rules (`setWorkflow(false)`), or the property was cleared. *Resolve* it in Statusbar. |
