# Statusbar for Azure Monitor

A firing Azure Monitor alert opens a message on your users' desktops; when Azure resolves the alert, the
message gets its green all-clear and disappears. Statusbar reads Azure's **common alert schema** as it
is, so all you configure is a webhook in an action group.

Three kinds of Azure alerts are useful for users:

| Azure alert | Typical use | Problem → recovery |
|---|---|---|
| **Metric and log search alerts** (stateful) | your own VMs, App Services, databases, Application Insights availability tests | `monitorCondition` *Fired* → *Resolved* |
| **Service Health** | Microsoft's own incidents and maintenance in your regions ("Azure SQL in West Europe has issues") | `stage` *Active* / *Planned* → *Resolved*, *Complete*, *RCA* |
| **Resource Health** | a single resource becomes unavailable | `currentHealthStatus` *Unavailable* / *Degraded* → *Available* |

| File | Purpose |
|---|---|
| [`test-fired.json`](test-fired.json), [`test-resolved.json`](test-resolved.json) | Metric alert, firing and resolved |
| [`test-servicehealth-active.json`](test-servicehealth-active.json), [`test-servicehealth-resolved.json`](test-servicehealth-resolved.json) | Service Health incident, active and resolved |

Tested with the sample payloads above, which follow Microsoft's published samples for the common alert
schema, and with the parser tests in `server/src/hooks.rs`. **Not tested against a live Azure
subscription.** If something does not behave as described, please send the payload from the alert's
history (see Troubleshooting).

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `Azure`, policy *Hold for approval*, delay `60` (`0` while testing).
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

Statusbar must be reachable from the internet over HTTPS with a publicly trusted certificate: Azure sends
from its own data centres. If you filter by source address, allow the Azure service tag **ActionGroup**.

## 2. Set up Azure

### 2.1 Action group

*Monitor → Alerts → Action groups → + Create*:

1. *Basics*: subscription, resource group, region *Global*, name `Statusbar`, display name `Statusbar`.
2. *Notifications*: none needed.
3. *Actions*: action type **Webhook**, name `Statusbar`:
   * URI: the webhook URL from step 1 (with the token – Azure webhooks cannot send a header)
   * **Enable the common alert schema: Yes** – without it Statusbar cannot read the alert
     (it answers `{"events":[],"ok":true}`)
   * leave *Secure webhook* off
4. *Review + create*.

Use a separate action group for Statusbar so you can attach it only to the alerts users should see.
Azure CLI, if you prefer:

```bash
az monitor action-group create -g rg-monitoring -n Statusbar --short-name Statusbar \
  --action webhook Statusbar "$(cat ~/statusbar-webhook-url)" usecommonalertschema
```

### 2.2 Metric and log search alerts

On an existing or new alert rule (*Monitor → Alerts → Alert rules*):

* *Actions*: add the action group **Statusbar**.
* *Details*:
  * **Severity**: `0 – Critical` and `1 – Error` → Outage, `2 – Warning` → Degraded,
    `3 – Informational` and `4 – Verbose` → Notice.
  * **Alert rule name** becomes the service and part of the title, **Description** the message text.
    Write both for people, or use a rule in Statusbar (step 5).
  * Log search alerts: tick **Automatically resolve alerts**. Without it the alert is *stateless*: Azure
    never sends *Resolved* and the message would stay until someone resolves it by hand.

Metric alerts are stateful by default.

### 2.3 Service Health

*Monitor → Service Health → Health alerts → + Create service health alert*:

1. *Condition*: subscription, the **services** your users depend on (e.g. *Virtual Machines*,
   *SQL Database*, *App Service*), your **regions**, and the event types:
   * **Service issue** – incidents (shown as Degraded). Recommended.
   * **Planned maintenance** – shown as Notice from the first announcement until it is complete. Only if
     users should know in advance.
   * *Health advisories*, *Security advisory* – usually only interesting for IT; leave them off.
2. *Actions*: action group **Statusbar**.
3. *Details*: name e.g. `Azure service health for users`.

Azure sends an update for every new communication about an incident; all of them carry the same
tracking ID, so Statusbar keeps one message and answers further updates with `duplicate`. The update
with stage *Resolved* (or *RCA*, *Complete*, *Canceled*) resolves it.

### 2.4 Resource Health (optional)

*Monitor → Service Health → Resource health → + Add resource health alert*: choose the resources,
*Current resource status* **Unavailable** and **Degraded** and also **Available** (that is the
recovery), action group **Statusbar**.

## 3. How Azure maps to Statusbar

| Azure (common alert schema) | Statusbar |
|---|---|
| `essentials.monitorCondition` = `Fired` / `Resolved` | problem / recovery (metric, log, Prometheus alerts) |
| `essentials.alertId` | key – identical in *Fired* and *Resolved* |
| `essentials.severity` `Sev0`, `Sev1` / `Sev2` / `Sev3`, `Sev4` | Outage / Degraded / Notice |
| `essentials.alertRule` | service; title `rule (resource)` |
| `essentials.configurationItems[0]` (else last part of `alertTargetIDs[0]`) | host |
| `essentials.description` | message text |
| `alertContext.annotations.summary` (Prometheus alerts) | title |
| Service Health `properties.stage` | `Resolved`, `RCA`, `Complete`, `Canceled` = recovery, everything else problem |
| Service Health `properties.trackingId` | key `azure-sh-<tracking id>` |
| Service Health `properties.title` / `service` / `region` | title / service / host |
| Service Health `properties.defaultLanguageContent` | message text (HTML removed) |
| Service Health `incidentType` = `Incident` / anything else | Degraded / Notice |
| Resource Health `currentHealthStatus` | `Unavailable` Outage, `Degraded` Degraded, `Available` recovery; key `azure-rh-<resource id>` |
| Other activity log alerts (administrative, policy, autoscale, security) | ignored – they never resolve |

## 4. Test

**Test the action group** – *Action groups → Statusbar → Test action group*, sample type e.g.
*Metric alert – Static threshold*. Azure sends one sample alert; depending on the sample it is *Fired*
(Statusbar shows a draft – *Discard* it under *Messages → Waiting for approval*) or *Resolved*
(`ignored`). Both prove URL, token, TLS and network. *Test action group* shows *Success* or the HTTP status.

**Sample payloads** – a full cycle without Azure:

```bash
URL='https://status.example.com/api/hooks/<token>'
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-fired.json
# {"events":["draft"],"ok":true}
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-resolved.json
# {"events":["cancelled"],"ok":true}
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-servicehealth-active.json
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-servicehealth-resolved.json
```

**End to end** – a metric alert on a test VM, *Percentage CPU* greater than `0`, aggregation granularity
1 minute, action group Statusbar. It fires within a few minutes; raise the threshold to `100` and Azure
resolves it.

## 5. Example rules

In Statusbar (*Monitoring → Add rule*):

| When host/service matches | Show this title | And this message | Severity |
|---|---|---|---|
| `vm-intranet*/*` | `The intranet is slow or not reachable` | `We are working on it.` | As reported |
| `*/SQL Database` | `Our business applications may be disrupted` | `Microsoft is working on an issue with Azure SQL in {host}. {message}` | As reported |

The subject is `<resource>/<alert rule name>` for your own alerts and `<region>/<Azure service>` for
Service Health, e.g. `West Europe/SQL Database`.

## Troubleshooting

Every alert keeps its notification history: *Monitor → Alerts →* open the alert *→ History*. The action
group's runs are under *Action groups → Statusbar → Activity log*.

| Symptom | Fix |
|---|---|
| `{"events":[],"ok":true}` | Common alert schema is off on the webhook action, or the alert is an activity log alert other than Service/Resource Health. |
| Test shows `401` | URL incomplete or integration deleted. |
| Test fails with a TLS or connection error | Statusbar is not reachable from the internet, its certificate is not publicly trusted, or a firewall blocks the *ActionGroup* service tag. |
| Message never resolves | Log search alert without *Automatically resolve alerts*; Resource Health alert without status *Available*; or the alert rule was disabled/deleted while firing. *Resolve* it in Statusbar. |
| Every Service Health update is a new message | Not expected – the tracking ID is the key. Check whether the updates come from different alert rules sent to different integrations. |
| Planned maintenance shows for days | Remove *Planned maintenance* from the Service Health alert, or give it a rule with recipients limited to IT. |
